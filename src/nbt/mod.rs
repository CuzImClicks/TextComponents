pub use crate::parse::nbt::{ComponentDecodeError, DecodeError};

const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;

mod encode;
mod snbt;
mod splice;
mod writers;

pub use encode::NbtBuilder;
pub use snbt::ToSNBT;
pub use splice::{Mutf8Fmt, SpliceComponent, SplicePart, splice_bytes, splice_len};
pub use writers::{
    nbt_len, write_color, write_component_elem, write_component_entry, write_component_root,
    write_encoded_elem, write_encoded_entry, write_encoded_root, write_mutf8, write_mutf8_body,
};
