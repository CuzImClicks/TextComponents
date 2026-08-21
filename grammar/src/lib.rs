//! The `MiniMessage` grammar: one parser shared by every consumer.

#[doc(hidden)]
pub mod nbt;

mod error;
mod ir;
mod parser;
mod tags;

pub use error::ParseError;
#[doc(hidden)]
pub use ir::*;
pub use parser::{Mode, parse};
pub use tags::NAMED_COLORS;
