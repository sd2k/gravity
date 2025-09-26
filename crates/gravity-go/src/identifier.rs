use crate::formatter::FormatInto;
use genco::lang::Go;
use genco::prelude::*;
use heck::{ToLowerCamelCase, ToUpperCamelCase};
use std::str::Chars;

/// Represents a Go identifier with appropriate casing rules.
///
/// Go identifiers follow specific naming conventions:
/// - Public identifiers start with uppercase (exported)
/// - Private identifiers start with lowercase (unexported)
/// - Local identifiers are used as-is without transformation
#[derive(Debug, Clone)]
pub enum GoIdentifier<'a> {
    /// Public/exported identifier (will be converted to UpperCamelCase)
    Public { name: &'a str },
    /// Private/unexported identifier (will be converted to lowerCamelCase)
    Private { name: &'a str },
    /// Local identifier (used as-is without case transformation)
    Local { name: &'a str },
}

impl<'a> GoIdentifier<'a> {
    /// Returns an iterator over the characters of the underlying name.
    ///
    /// This provides access to the raw name without case transformations.
    ///
    /// # Returns
    /// An iterator over the characters of the identifier's name.
    pub fn chars(&self) -> Chars<'a> {
        match self {
            GoIdentifier::Public { name } => name.chars(),
            GoIdentifier::Private { name } => name.chars(),
            GoIdentifier::Local { name } => name.chars(),
        }
    }
}

impl From<GoIdentifier<'_>> for String {
    fn from(value: GoIdentifier) -> Self {
        let mut tokens: Tokens<Go> = Tokens::new();
        value.format_into(&mut tokens);
        tokens.to_string().expect("to format correctly")
    }
}

impl FormatInto<Go> for GoIdentifier<'_> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            GoIdentifier::Public { name } => {
                let formatted = name.to_upper_camel_case();
                tokens.append(formatted);
            }
            GoIdentifier::Private { name } => {
                let formatted = name.to_lower_camel_case();
                tokens.append(formatted);
            }
            GoIdentifier::Local { name } => {
                tokens.append(name);
            }
        }
    }
}
