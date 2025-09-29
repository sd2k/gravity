//! Representations of Go types, and implementations for formatting them.

mod comment;
mod embed;
#[path = "./type.rs"]
mod go_type;
mod identifier;
mod operand;
mod result;

pub use comment::*;
pub use embed::*;
pub use go_type::*;
pub use identifier::*;
pub use operand::*;
pub use result::*;
