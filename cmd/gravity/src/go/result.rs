use genco::prelude::*;

use crate::go::GoType;

/// Represents a Go function result type.
///
/// Can be either empty (no return value) or an anonymous type.
/// Used for modeling function returns in the generated Go code.
#[derive(Debug, Clone, PartialEq)]
pub enum GoResult {
    /// No return value
    Empty,
    /// Anonymous return type
    Anon(GoType),
}

impl GoResult {
    /// Returns true if this result type needs post-return cleanup.
    ///
    /// Delegates to the underlying type's cleanup requirements.
    /// Empty results don't need cleanup as they represent no value.
    ///
    /// # Returns
    /// `true` if cleanup is needed, `false` otherwise.
    pub fn needs_cleanup(&self) -> bool {
        match self {
            GoResult::Empty => false,
            GoResult::Anon(typ) => typ.needs_cleanup(),
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

#[cfg(test)]
mod tests {
    use genco::{prelude::*, tokens::Tokens};

    use crate::go::{GoResult, GoType};

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
}
