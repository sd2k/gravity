pub mod embed;
pub mod formatter;
pub mod identifier;
pub mod types;

pub use embed::{embed, Embed};
pub use identifier::GoIdentifier;
pub use types::{GoResult, GoType, Operand};

// Re-export genco types that are commonly used
pub use genco::{lang::Go, quote, Tokens};
