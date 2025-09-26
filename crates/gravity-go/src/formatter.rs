use crate::types::{GoResult, GoType, Operand};
use genco::lang::{Go, Lang};
use genco::prelude::*;
use genco::tokens::ItemStr;

pub trait FormatInto<L: Lang> {
    fn format_into(self, tokens: &mut Tokens<L>);
}

impl FormatInto<Go> for GoType {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

impl FormatInto<Go> for &GoType {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            GoType::Bool => tokens.append("bool"),
            GoType::Uint8 => tokens.append("uint8"),
            GoType::Uint16 => tokens.append("uint16"),
            GoType::Uint32 => tokens.append("uint32"),
            GoType::Uint64 => tokens.append("uint64"),
            GoType::Int8 => tokens.append("int8"),
            GoType::Int16 => tokens.append("int16"),
            GoType::Int32 => tokens.append("int32"),
            GoType::Int64 => tokens.append("int64"),
            GoType::Float32 => tokens.append("float32"),
            GoType::Float64 => tokens.append("float64"),
            GoType::String => tokens.append("string"),
            GoType::Error => tokens.append("error"),
            GoType::Interface => tokens.append("interface{}"),
            GoType::Pointer(inner) => {
                tokens.append("*");
                inner.as_ref().format_into(tokens);
            }
            GoType::ValueOrOk(inner) => {
                tokens.append("(");
                inner.as_ref().format_into(tokens);
                tokens.append(", bool)");
            }
            GoType::ValueOrError(inner) => {
                tokens.append("(");
                inner.as_ref().format_into(tokens);
                tokens.append(", error)");
            }
            GoType::Slice(inner) => {
                tokens.append("[]");
                inner.as_ref().format_into(tokens);
            }
            GoType::UserDefined(name) => tokens.append(name.as_str()),
            GoType::Nothing => {}
        }
    }
}

impl FormatInto<Go> for GoResult {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

impl FormatInto<Go> for &GoResult {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            GoResult::Empty => tokens.append("error"),
            GoResult::Anon(typ) => {
                tokens.append("(");
                typ.format_into(tokens);
                tokens.append(", error)");
            }
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
                tokens.append(",");
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
