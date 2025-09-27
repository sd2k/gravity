pub mod bindings;
pub mod context;
pub mod exports;
pub mod factory;
pub mod imports;
pub mod instructions;
pub mod types;

use gravity_go::GoType;
use wit_bindgen_core::wit_parser::{Resolve, Type, TypeDefKind};

pub use bindings::BindingsGenerator;
pub use context::GenerationContext;
pub use factory::{FactoryConfig, FactoryGenerator};
pub use imports::{generate_imports_with_chains, ImportResult};
pub use instructions::{handle_instruction, InstructionHandler};
pub use types::TypeGenerator;

/// Resolves a WIT type to a Go type.
pub fn resolve_type(typ: &Type, resolve: &Resolve) -> anyhow::Result<GoType> {
    Ok(match typ {
        Type::Bool => GoType::Bool,
        Type::U8 => GoType::Uint8,
        Type::U16 => GoType::Uint16,
        Type::U32 => GoType::Uint32,
        Type::U64 => GoType::Uint64,
        Type::S8 => GoType::Int8,
        Type::S16 => GoType::Int16,
        Type::S32 => GoType::Int32,
        Type::S64 => GoType::Int64,
        Type::F32 => GoType::Float32,
        Type::F64 => GoType::Float64,
        Type::String => GoType::String,
        Type::Char => GoType::Uint32, // Char is represented as uint32
        Type::ErrorContext => GoType::Interface, // TODO: Handle ErrorContext properly
        Type::Id(id) => {
            let typedef = resolve.types.get(*id).unwrap();
            match &typedef.kind {
                TypeDefKind::List(inner) => GoType::Slice(Box::new(resolve_type(inner, resolve)?)),
                TypeDefKind::Option(inner) => {
                    GoType::ValueOrOk(Box::new(resolve_type(inner, resolve)?))
                }
                TypeDefKind::Result(result) => match (&result.ok, &result.err) {
                    (Some(ok), None) => GoType::ValueOrOk(Box::new(resolve_type(ok, resolve)?)),
                    (Some(ok), Some(_err)) => {
                        // Result<T, E> is represented as (T, error) in Go
                        GoType::ValueOrError(Box::new(resolve_type(ok, resolve)?))
                    }
                    (None, Some(_err)) => {
                        // Result<(), E> is represented as error in Go
                        GoType::Error
                    }
                    (None, None) => GoType::Nothing, // Result<(), ()> has no meaningful representation
                },
                TypeDefKind::Variant(_) => GoType::UserDefined(
                    typedef
                        .name
                        .clone()
                        .unwrap_or_else(|| "Anonymous".to_string()),
                ),
                TypeDefKind::Enum(_) => GoType::Uint32, // Enums are represented as integers
                TypeDefKind::Record(_) | TypeDefKind::Flags(_) | TypeDefKind::Tuple(_) => {
                    GoType::UserDefined(
                        typedef
                            .name
                            .clone()
                            .unwrap_or_else(|| "Anonymous".to_string()),
                    )
                }
                TypeDefKind::Type(t) => resolve_type(t, resolve)?,
                _ => GoType::Interface,
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wit_bindgen_core::wit_parser::{Resolve, Type};

    #[test]
    fn test_resolve_primitive_types() {
        let resolve = Resolve::default();

        assert_eq!(resolve_type(&Type::Bool, &resolve).unwrap(), GoType::Bool);
        assert_eq!(resolve_type(&Type::U8, &resolve).unwrap(), GoType::Uint8);
        assert_eq!(resolve_type(&Type::U16, &resolve).unwrap(), GoType::Uint16);
        assert_eq!(resolve_type(&Type::U32, &resolve).unwrap(), GoType::Uint32);
        assert_eq!(resolve_type(&Type::U64, &resolve).unwrap(), GoType::Uint64);
        assert_eq!(resolve_type(&Type::S8, &resolve).unwrap(), GoType::Int8);
        assert_eq!(resolve_type(&Type::S16, &resolve).unwrap(), GoType::Int16);
        assert_eq!(resolve_type(&Type::S32, &resolve).unwrap(), GoType::Int32);
        assert_eq!(resolve_type(&Type::S64, &resolve).unwrap(), GoType::Int64);
        assert_eq!(resolve_type(&Type::F32, &resolve).unwrap(), GoType::Float32);
        assert_eq!(resolve_type(&Type::F64, &resolve).unwrap(), GoType::Float64);
        assert_eq!(
            resolve_type(&Type::String, &resolve).unwrap(),
            GoType::String
        );
    }

    #[test]
    fn test_resolve_char_type() {
        let resolve = Resolve::default();
        // Char is represented as uint32 in Go
        assert_eq!(resolve_type(&Type::Char, &resolve).unwrap(), GoType::Uint32);
    }

    #[test]
    fn test_resolve_error_context_type() {
        let resolve = Resolve::default();
        // ErrorContext is represented as interface{} for now
        assert_eq!(
            resolve_type(&Type::ErrorContext, &resolve).unwrap(),
            GoType::Interface
        );
    }
}
