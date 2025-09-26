use crate::identifier::GoIdentifier;
use crate::types::{GoResult, GoType, Operand};
use genco::lang::{Go, Lang};
use genco::prelude::*;
use genco::tokens::{static_literal, ItemStr};

/// Trait for formatting types into language-specific tokens.
///
/// This trait allows types to be formatted into token streams
/// for code generation. It's primarily used for Go code generation
/// but is generic over the language type.
pub trait FormatInto<L: Lang> {
    /// Formats the type into the provided token stream.
    ///
    /// # Arguments
    /// * `tokens` - The token stream to append formatted output to.
    fn format_into(self, tokens: &mut Tokens<L>);
}

// Implement only genco's FormatInto trait (not our own) to avoid conflicts
impl genco::prelude::FormatInto<Go> for GoType {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

impl genco::prelude::FormatInto<Go> for &GoType {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            GoType::Bool => tokens.append(static_literal("bool")),
            GoType::Uint8 => tokens.append(static_literal("uint8")),
            GoType::Uint16 => tokens.append(static_literal("uint16")),
            GoType::Uint32 => tokens.append(static_literal("uint32")),
            GoType::Uint64 => tokens.append(static_literal("uint64")),
            GoType::Int8 => tokens.append(static_literal("int8")),
            GoType::Int16 => tokens.append(static_literal("int16")),
            GoType::Int32 => tokens.append(static_literal("int32")),
            GoType::Int64 => tokens.append(static_literal("int64")),
            GoType::Float32 => tokens.append(static_literal("float32")),
            GoType::Float64 => tokens.append(static_literal("float64")),
            GoType::String => tokens.append(static_literal("string")),
            GoType::Error => tokens.append(static_literal("error")),
            GoType::Interface => tokens.append(static_literal("interface{}")),
            GoType::ValueOrOk(value_typ) => {
                value_typ.as_ref().format_into(tokens);
                tokens.append(static_literal(","));
                tokens.space();
                tokens.append(static_literal("bool"))
            }
            GoType::ValueOrError(value_typ) => {
                value_typ.as_ref().format_into(tokens);
                tokens.append(static_literal(","));
                tokens.space();
                tokens.append(static_literal("error"))
            }
            GoType::Slice(typ) => {
                tokens.append(static_literal("[]"));
                typ.as_ref().format_into(tokens);
            }
            GoType::Pointer(typ) => {
                tokens.append(static_literal("*"));
                typ.as_ref().format_into(tokens);
            }
            GoType::UserDefined(name) => {
                let id = GoIdentifier::Public { name };
                id.format_into(tokens)
            }
            GoType::Nothing => (),
        }
    }
}

impl genco::prelude::FormatInto<Go> for GoResult {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

impl genco::prelude::FormatInto<Go> for &GoResult {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match &self {
            GoResult::Anon(typ @ GoType::ValueOrError(_) | typ @ GoType::ValueOrOk(_)) => {
                // Be cautious here as there are `(` and `)` surrounding the type
                tokens.append(quote!(($typ)))
            }
            GoResult::Anon(typ) => typ.format_into(tokens),
            GoResult::Empty => (),
        }
    }
}

// Implement genco's FormatInto for Operand so it can be used in quote! macros
impl genco::prelude::FormatInto<Go> for &Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            Operand::Literal(val) => tokens.append(ItemStr::from(val)),
            Operand::SingleValue(val) => tokens.append(ItemStr::from(val)),
            Operand::MultiValue((val1, val2)) => {
                tokens.append(ItemStr::from(val1));
                tokens.append(static_literal(","));
                tokens.space();
                tokens.append(ItemStr::from(val2));
            }
        }
    }
}

impl genco::prelude::FormatInto<Go> for Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

impl genco::prelude::FormatInto<Go> for &mut Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        let op: &Operand = self;
        op.format_into(tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{GoResult, GoType, Operand};
    use genco::prelude::FormatInto;

    #[test]
    fn test_basic_types() {
        let cases = vec![
            (GoType::Bool, "bool"),
            (GoType::Uint8, "uint8"),
            (GoType::Uint16, "uint16"),
            (GoType::Uint32, "uint32"),
            (GoType::Uint64, "uint64"),
            (GoType::Int8, "int8"),
            (GoType::Int16, "int16"),
            (GoType::Int32, "int32"),
            (GoType::Int64, "int64"),
            (GoType::Float32, "float32"),
            (GoType::Float64, "float64"),
            (GoType::String, "string"),
            (GoType::Error, "error"),
            (GoType::Interface, "interface{}"),
            (GoType::Nothing, ""),
        ];

        for (typ, expected) in cases {
            let mut tokens = Tokens::<Go>::new();
            (&typ).format_into(&mut tokens);
            assert_eq!(
                tokens.to_string().unwrap(),
                expected,
                "Failed for type: {:?}",
                typ
            );
        }
    }

    #[test]
    fn test_value_or_ok() {
        let typ = GoType::ValueOrOk(Box::new(GoType::Uint32));
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "uint32, bool");
    }

    #[test]
    fn test_value_or_error() {
        let typ = GoType::ValueOrError(Box::new(GoType::String));
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "string, error");
    }

    #[test]
    fn test_slice() {
        let typ = GoType::Slice(Box::new(GoType::Int32));
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "[]int32");
    }

    #[test]
    fn test_pointer() {
        let typ = GoType::Pointer(Box::new(GoType::String));
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "*string");
    }

    #[test]
    fn test_nested_types() {
        // Test *[]string
        let typ = GoType::Pointer(Box::new(GoType::Slice(Box::new(GoType::String))));
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "*[]string");

        // Test [][]uint8
        let typ = GoType::Slice(Box::new(GoType::Slice(Box::new(GoType::Uint8))));
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "[][]uint8");
    }

    #[test]
    fn test_go_result_empty() {
        let result = GoResult::Empty;
        let mut tokens = Tokens::<Go>::new();
        (&result).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "");
    }

    #[test]
    fn test_go_result_simple_type() {
        let result = GoResult::Anon(GoType::String);
        let mut tokens = Tokens::<Go>::new();
        (&result).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "string");
    }

    #[test]
    fn test_go_result_value_or_ok() {
        // GoResult with ValueOrOk should add parentheses
        let result = GoResult::Anon(GoType::ValueOrOk(Box::new(GoType::Uint32)));
        let mut tokens = Tokens::<Go>::new();
        (&result).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "(uint32, bool)");
    }

    #[test]
    fn test_go_result_value_or_error() {
        // GoResult with ValueOrError should add parentheses
        let result = GoResult::Anon(GoType::ValueOrError(Box::new(GoType::String)));
        let mut tokens = Tokens::<Go>::new();
        (&result).format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "(string, error)");
    }

    #[test]
    fn test_operand_literal() {
        let op = Operand::Literal("42".to_string());
        let mut tokens = Tokens::<Go>::new();
        op.format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "42");
    }

    #[test]
    fn test_operand_single_value() {
        let op = Operand::SingleValue("myVar".to_string());
        let mut tokens = Tokens::<Go>::new();
        op.format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "myVar");
    }

    #[test]
    fn test_operand_multi_value() {
        let op = Operand::MultiValue(("val1".to_string(), "val2".to_string()));
        let mut tokens = Tokens::<Go>::new();
        op.format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "val1, val2");
    }

    #[test]
    fn test_user_defined_type() {
        // User-defined types should be formatted with proper casing
        let typ = GoType::UserDefined("myCustomType".to_string());
        let mut tokens = Tokens::<Go>::new();
        (&typ).format_into(&mut tokens);
        // This should use GoIdentifier formatting which capitalizes the first letter
        assert_eq!(tokens.to_string().unwrap(), "MyCustomType");
    }
}
