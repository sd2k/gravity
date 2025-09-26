use anyhow::Result;
use genco::prelude::*;
use gravity_go::{Go, Tokens};
use wit_bindgen_core::wit_parser::{Function, TypeDef};

/// Generator for WebAssembly Interface Types (WIT) bindings in Go.
///
/// This struct accumulates type definitions, function bindings, and imports,
/// then generates the corresponding Go code.
pub struct BindingsGenerator {
    /// Accumulated output tokens for the generated Go code.
    output: Tokens<Go>,
    /// Collection of type definitions to be generated.
    types: Vec<TypeDef>,
}

impl Default for BindingsGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl BindingsGenerator {
    /// Creates a new empty bindings generator.
    ///
    /// Initializes the generator with empty output and no types.
    pub fn new() -> Self {
        Self {
            output: Tokens::new(),
            types: Vec::new(),
        }
    }

    /// Adds a type definition to be generated.
    ///
    /// Stores the type definition and generates a placeholder comment in the output.
    /// The actual type generation is marked as TODO.
    ///
    /// # Arguments
    /// * `type_def` - The WIT type definition to add.
    ///
    /// # Returns
    /// Ok(()) on success.
    pub fn add_type(&mut self, type_def: &TypeDef) -> Result<()> {
        self.types.push(type_def.clone());
        // TODO: Generate type definition
        let type_name = type_def.name.as_deref().unwrap_or("anonymous");
        self.output.line();
        self.output.append(format!("// Type: {}", type_name));
        Ok(())
    }

    /// Adds a function binding to be generated.
    ///
    /// Generates a placeholder comment for the function in the output.
    /// The actual function binding generation is marked as TODO.
    ///
    /// # Arguments
    /// * `func` - The WIT function to add bindings for.
    ///
    /// # Returns
    /// Ok(()) on success.
    pub fn add_function(&mut self, func: &Function) -> Result<()> {
        // TODO: Generate function binding
        self.output.line();
        self.output.append(format!("// Function: {}", func.name));
        Ok(())
    }

    /// Adds an import statement with the specified items.
    ///
    /// Generates a Go import block with the provided items.
    ///
    /// # Arguments
    /// * `_module` - The module name (currently unused).
    /// * `items` - List of items to import.
    ///
    /// # Returns
    /// Ok(()) on success.
    pub fn add_import(&mut self, _module: &str, items: Vec<String>) -> Result<()> {
        quote_in! { self.output =>
            import (
                $(for item in items join ($['\r']) => $(quoted(item)))
            )
            $['\n']
        }
        Ok(())
    }

    /// Generates the final Go code from all accumulated definitions.
    ///
    /// Consumes the generator and returns the generated Go code as a string.
    ///
    /// # Returns
    /// The generated Go code.
    ///
    /// # Panics
    /// Panics if code generation fails.
    pub fn generate(self) -> String {
        self.output.to_string().expect("Failed to generate Go code")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wit_bindgen_core::wit_parser::{TypeDefKind, TypeOwner};

    fn create_test_typedef(name: &str) -> TypeDef {
        TypeDef {
            name: Some(name.to_string()),
            kind: TypeDefKind::Type(wit_bindgen_core::wit_parser::Type::Bool),
            owner: TypeOwner::None,
            docs: Default::default(),
            stability: Default::default(),
        }
    }

    #[test]
    fn test_new_generator() {
        let generator = BindingsGenerator::new();
        assert_eq!(generator.types.len(), 0);
        let output = generator.generate();
        assert_eq!(output, "");
    }

    #[test]
    fn test_add_type() {
        let mut generator = BindingsGenerator::new();
        let type_def = create_test_typedef("MyType");

        generator.add_type(&type_def).unwrap();
        assert_eq!(generator.types.len(), 1);

        let output = generator.generate();
        assert!(output.contains("// Type: MyType"));
    }

    #[test]
    fn test_add_multiple_types() {
        let mut generator = BindingsGenerator::new();

        generator.add_type(&create_test_typedef("Type1")).unwrap();
        generator.add_type(&create_test_typedef("Type2")).unwrap();
        generator.add_type(&create_test_typedef("Type3")).unwrap();

        assert_eq!(generator.types.len(), 3);

        let output = generator.generate();
        assert!(output.contains("// Type: Type1"));
        assert!(output.contains("// Type: Type2"));
        assert!(output.contains("// Type: Type3"));
    }

    #[test]
    fn test_add_import() {
        let mut generator = BindingsGenerator::new();

        generator
            .add_import("fmt", vec!["Println".to_string(), "Sprintf".to_string()])
            .unwrap();

        let output = generator.generate();
        assert!(output.contains("import ("));
        assert!(output.contains("\"Println\""));
        assert!(output.contains("\"Sprintf\""));
    }

    #[test]
    fn test_add_function() {
        let mut generator = BindingsGenerator::new();

        // Create a test function
        let func = wit_bindgen_core::wit_parser::Function {
            name: "testFunc".to_string(),
            kind: wit_bindgen_core::wit_parser::FunctionKind::Freestanding,
            params: Default::default(),
            result: Default::default(),
            docs: Default::default(),
            stability: Default::default(),
        };

        generator.add_function(&func).unwrap();

        let output = generator.generate();
        assert!(output.contains("// Function: testFunc"));
    }

    #[test]
    fn test_complete_generation() {
        let mut generator = BindingsGenerator::new();

        // Add various elements
        generator
            .add_import("fmt", vec!["Println".to_string()])
            .unwrap();
        generator.add_type(&create_test_typedef("User")).unwrap();
        generator.add_type(&create_test_typedef("Product")).unwrap();

        let func = wit_bindgen_core::wit_parser::Function {
            name: "processUser".to_string(),
            kind: wit_bindgen_core::wit_parser::FunctionKind::Freestanding,
            params: Default::default(),
            result: Default::default(),
            docs: Default::default(),
            stability: Default::default(),
        };
        generator.add_function(&func).unwrap();

        let output = generator.generate();

        // Verify all components are in the output
        assert!(output.contains("import"));
        assert!(output.contains("Println"));
        assert!(output.contains("// Type: User"));
        assert!(output.contains("// Type: Product"));
        assert!(output.contains("// Function: processUser"));
    }

    #[test]
    fn test_anonymous_type() {
        let mut generator = BindingsGenerator::new();

        let type_def = TypeDef {
            name: None, // Anonymous type
            kind: TypeDefKind::Type(wit_bindgen_core::wit_parser::Type::String),
            owner: TypeOwner::None,
            docs: Default::default(),
            stability: Default::default(),
        };

        generator.add_type(&type_def).unwrap();

        let output = generator.generate();
        assert!(output.contains("// Type: anonymous"));
    }
}
