use genco::lang::Go;
use genco::tokens::{static_literal, FormatInto, ItemStr, Tokens};

/// Type for generating Go embed directives (//go:embed)
pub struct Embed<T>(pub T);

impl<T> FormatInto<Go> for Embed<T>
where
    T: Into<ItemStr>,
{
    fn format_into(self, tokens: &mut Tokens<Go>) {
        // TODO(#13): Submit patch to genco that will allow aliases for go imports
        // tokens.register(go::import("embed", ""));
        tokens.push();
        tokens.append(static_literal("//go:embed"));
        tokens.space();
        tokens.append(self.0.into());
    }
}

/// Helper function to create an embed directive
pub fn embed<T>(path: T) -> Embed<T>
where
    T: Into<ItemStr>,
{
    Embed(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use genco::prelude::*;

    #[test]
    fn test_embed_directive() {
        let mut tokens = Tokens::<Go>::new();
        let embed = embed("module.wasm");

        quote_in! { tokens =>
            $(embed)
        };

        let output = tokens.to_string().unwrap();
        assert!(output.contains("//go:embed module.wasm"));
    }

    #[test]
    fn test_embed_with_variable() {
        let mut tokens = Tokens::<Go>::new();

        quote_in! { tokens =>
            $(embed("app.wasm"))
            var wasmFile []byte
        };

        let output = tokens.to_string().unwrap();
        assert!(output.contains("//go:embed app.wasm"));
        assert!(output.contains("var wasmFile []byte"));
    }
}
