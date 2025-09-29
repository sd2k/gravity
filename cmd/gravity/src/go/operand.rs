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
    /// A tuple of two values (for multi-value returns)
    MultiValue((String, String)),
}

impl Operand {
    /// Returns the primary value of the operand.
    ///
    /// For single values and literals, returns the value itself.
    /// For multi-value tuples, returns the first value.
    ///
    /// # Returns
    /// A string representation of the primary value.
    pub fn as_string(&self) -> String {
        match self {
            Operand::Literal(s) => s.clone(),
            Operand::SingleValue(s) => s.clone(),
            Operand::MultiValue((s1, _)) => s1.clone(),
        }
    }
}

// Implement genco's FormatInto for Operand so it can be used in quote! macros
impl FormatInto<Go> for &Operand {
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
    fn test_operand_multi_value() {
        let op = Operand::MultiValue(("val1".to_string(), "val2".to_string()));
        let mut tokens = Tokens::<Go>::new();
        op.format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "val1, val2");
    }
}
