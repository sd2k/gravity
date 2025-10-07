use genco::{prelude::*, tokens::static_literal};

#[derive(Debug, Clone, Default)]
/// Doc comments.
pub struct GoDoc(Option<String>);

impl GoDoc {
    pub fn new(docs: Option<String>) -> Self {
        Self(docs)
    }
}

impl From<Option<String>> for GoDoc {
    fn from(value: Option<String>) -> Self {
        Self::new(value)
    }
}

impl From<Option<&String>> for GoDoc {
    fn from(value: Option<&String>) -> Self {
        Self::new(value.map(|s| s.clone()))
    }
}

impl From<Option<&str>> for GoDoc {
    fn from(value: Option<&str>) -> Self {
        Self::new(value.map(|s| s.to_string()))
    }
}

impl<T> From<&T> for GoDoc
where
    T: Into<GoDoc> + Clone,
{
    fn from(value: &T) -> Self {
        value.clone().into()
    }
}

impl FormatInto<Go> for &GoDoc {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        if let Some(contents) = &self.0 {
            for line in contents.lines() {
                tokens.push();
                tokens.append(static_literal("//"));
                tokens.space();
                tokens.append(line);
            }
        }
    }
}

impl std::fmt::Display for GoDoc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(contents) = &self.0 {
            for line in contents.lines() {
                writeln!(f, "// {}", line)?;
            }
        }
        Ok(())
    }
}
