use crate::types::{GoResult, GoType, Operand};
use genco::lang::{Go, Lang};
use genco::prelude::*;

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

impl FormatInto<Go> for &Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            Operand::Literal(val) => tokens.append(val.as_str()),
            Operand::SingleValue(val) => tokens.append(val.as_str()),
            Operand::MultiValue((val1, val2)) => {
                tokens.append(val1.as_str());
                tokens.append(",");
                tokens.space();
                tokens.append(val2.as_str());
            }
        }
    }
}

impl FormatInto<Go> for Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}
