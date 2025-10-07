//! Representations of Go types, and implementations for formatting them.

mod comment;
mod docs;
mod embed;
#[path = "./type.rs"]
mod go_type;
mod identifier;
pub mod imports;
mod operand;
mod result;

pub use comment::*;
pub use docs::*;
pub use embed::*;
pub use go_type::*;
pub use identifier::*;
pub use operand::*;
pub use result::*;
