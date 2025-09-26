use anyhow::Result;
use genco::prelude::*;
use gravity_go::{quote, Go, Tokens};
use wit_parser::{Function, TypeDef};

pub struct BindingsGenerator {
    output: Tokens<Go>,
    types: Vec<TypeDef>,
}

impl BindingsGenerator {
    pub fn new() -> Self {
        Self {
            output: Tokens::new(),
            types: Vec::new(),
        }
    }

    pub fn add_type(&mut self, type_def: &TypeDef) -> Result<()> {
        self.types.push(type_def.clone());
        // TODO: Generate type definition
        quote_in! { self.output =>
            $['\n']
            // Type: $(type_def.name.as_ref().unwrap_or(&String::from("anonymous")))
        }
        Ok(())
    }

    pub fn add_function(&mut self, func: &Function) -> Result<()> {
        // TODO: Generate function binding
        quote_in! { self.output =>
            $['\n']
            // Function: $(&func.name)
        }
        Ok(())
    }

    pub fn add_import(&mut self, module: &str, items: Vec<String>) -> Result<()> {
        quote_in! { self.output =>
            import (
                $(for item in items join ($['\r']) => $(quoted(item)))
            )
            $['\n']
        }
        Ok(())
    }

    pub fn generate(self) -> String {
        self.output.to_string().expect("Failed to generate Go code")
    }
}
