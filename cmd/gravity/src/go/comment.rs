use genco::{
    prelude::*,
    tokens::{ItemStr, static_literal},
};

/// Format a comment where each line is preceeded by `//`.
/// Based on https://github.com/udoprog/genco/blob/1ec4869f458cf71d1d2ffef77fe051ea8058b391/src/lang/csharp/comment.rs
pub struct Comment<T>(T);

impl<T> FormatInto<Go> for Comment<T>
where
    T: IntoIterator,
    T::Item: Into<ItemStr>,
{
    fn format_into(self, tokens: &mut Tokens<Go>) {
        for line in self.0 {
            tokens.push();
            tokens.append(static_literal("//"));
            tokens.space();
            tokens.append(line.into());
        }
    }
}

/// Helper function to create a Go comment.
pub fn comment<T>(comment: T) -> Comment<T>
where
    T: IntoIterator,
    T::Item: Into<ItemStr>,
{
    Comment(comment)
}

#[cfg(test)]
mod tests {
    use genco::{prelude::*, tokens::Tokens};

    use crate::go::comment;

    #[test]
    fn test_comment() {
        let comment = comment(&["hello", "world"]);
        let mut tokens = Tokens::<Go>::new();
        comment.format_into(&mut tokens);
        assert_eq!(tokens.to_string().unwrap(), "// hello\n// world");
    }
}
