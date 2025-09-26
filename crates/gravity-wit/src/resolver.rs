use wit_bindgen_core::wit_parser::{Resolve, Type, TypeDef, TypeId};

/// Resolves a type ID to its definition
pub fn resolve_type(resolve: &Resolve, type_id: TypeId) -> Option<&TypeDef> {
    resolve.types.get(type_id)
}

/// Gets the name of a type, handling anonymous types
pub fn type_name(resolve: &Resolve, ty: &Type) -> String {
    match ty {
        Type::Bool => "bool".to_string(),
        Type::Char => "char".to_string(),
        Type::U8 => "uint8".to_string(),
        Type::U16 => "uint16".to_string(),
        Type::U32 => "uint32".to_string(),
        Type::U64 => "uint64".to_string(),
        Type::S8 => "int8".to_string(),
        Type::S16 => "int16".to_string(),
        Type::S32 => "int32".to_string(),
        Type::S64 => "int64".to_string(),
        Type::F32 => "float32".to_string(),
        Type::F64 => "float64".to_string(),
        Type::String => "string".to_string(),
        Type::Id(id) => {
            if let Some(type_def) = resolve_type(resolve, *id) {
                type_def
                    .name
                    .clone()
                    .unwrap_or_else(|| "anonymous".to_string())
            } else {
                "unknown".to_string()
            }
        }
    }
}
