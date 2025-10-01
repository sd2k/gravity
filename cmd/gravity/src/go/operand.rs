use genco::{
    prelude::*,
    tokens::{ItemStr, static_literal},
};

/// Represents an operand in Go code generation.
///
/// Operands can be literals, single values (variables), or multi-value tuples
/// (used for functions returning multiple values).
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// A literal value (e.g., "0", "true", "\"hello\"")
    Literal(String),
    /// A single variable or expression
    SingleValue(String),
    /// A tuple of two values (shortcut for for multi-value returns).
    ///
    /// This is used when returning `val, ok` or `result, err` from Go functions.
    DoubleValue(String, String),
    /// A tuple of two or more values (for tuples)
    MultiValue(Vec<String>),
}

// Implement genco's FormatInto for Operand so it can be used in quote! macros
impl FormatInto<Go> for &Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            Operand::Literal(val) => tokens.append(ItemStr::from(val)),
            Operand::SingleValue(val) => tokens.append(ItemStr::from(val)),
            Operand::DoubleValue(val, ok) => {
                tokens.append(ItemStr::from(val));
                tokens.append(static_literal(","));
                tokens.space();
                tokens.append(ItemStr::from(ok));
            }
            Operand::MultiValue(vals) => {
                if let Some((last, vals)) = vals.split_last() {
                    for val in vals.iter() {
                        tokens.append(val);
                        tokens.append(static_literal(","));
                        tokens.space();
                    }
                    tokens.append(last);
                }
            }
        }
    }
}

impl FormatInto<Go> for Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

impl FormatInto<Go> for &mut Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        let op: &Operand = self;
        op.format_into(tokens)
    }
}

#[cfg(test)]
mod tests {
    use genco::{prelude::*, tokens::Tokens};

    use crate::go::Operand;

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
    fn test_operand_double_value() {
        let op = Operand::DoubleValue("val1".to_string(), "val2".to_string());
        let mut tokens = Tokens::<Go>::new();
        op.format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "val1, val2");
    }

    #[test]
    fn test_operand_multi_value() {
        let op = Operand::MultiValue(vec![
            "val1".to_string(),
            "val2".to_string(),
            "val3".to_string(),
        ]);
        let mut tokens = Tokens::<Go>::new();
        op.format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "val1, val2, val3");
    }
}
