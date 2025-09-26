pub mod resolver;

use wit_bindgen_core::wit_parser::{Resolve, Type};

/// Helper functions for working with WIT types
pub fn size_of_type(resolve: &Resolve, ty: &Type) -> usize {
    // TODO: Implement proper size calculation
    match ty {
        Type::Bool | Type::U8 | Type::S8 | Type::Char => 1,
        Type::U16 | Type::S16 => 2,
        Type::U32 | Type::S32 | Type::F32 => 4,
        Type::U64 | Type::S64 | Type::F64 => 8,
        Type::String => std::mem::size_of::<(*const u8, usize)>(),
        Type::ErrorContext => std::mem::size_of::<(*const u8, usize)>(), // Similar to string
        Type::Id(id) => {
            // Look up the type definition and calculate its size
            // This is a placeholder - proper implementation needed
            8
        }
    }
}

pub fn align_of_type(resolve: &Resolve, ty: &Type) -> usize {
    // TODO: Implement proper alignment calculation
    match ty {
        Type::Bool | Type::U8 | Type::S8 | Type::Char => 1,
        Type::U16 | Type::S16 => 2,
        Type::U32 | Type::S32 | Type::F32 => 4,
        Type::U64 | Type::S64 | Type::F64 => 8,
        Type::String => std::mem::align_of::<(*const u8, usize)>(),
        Type::ErrorContext => std::mem::align_of::<(*const u8, usize)>(), // Similar to string
        Type::Id(id) => {
            // Look up the type definition and calculate its alignment
            // This is a placeholder - proper implementation needed
            8
        }
    }
}
