use anyhow::Result;
use genco::prelude::*;
use gravity_go::{Go, GoIdentifier};
use std::collections::BTreeMap;

use crate::context::GenerationContext;
use crate::imports::GoImports;

/// Configuration for factory generation
pub struct FactoryConfig<'a> {
    pub world_name: &'a str,
    pub go_imports: &'a GoImports,
    pub interface_params: Vec<String>,
    pub import_chains: BTreeMap<String, Tokens<Go>>,
    pub wasm_var_name: &'a GoIdentifier<'a>,
}

/// Generator for factory and instance types
pub struct FactoryGenerator<'a> {
    context: &'a mut GenerationContext,
    config: FactoryConfig<'a>,
}

impl<'a> FactoryGenerator<'a> {
    pub fn new(context: &'a mut GenerationContext, config: FactoryConfig<'a>) -> Self {
        Self { context, config }
    }

    /// Generate factory and instance types and methods
    pub fn generate(mut self) -> Result<()> {
        let world_name = self.config.world_name;

        // Create identifiers
        let factory = GoIdentifier::Public {
            name: &format!("{world_name}-factory"),
        };
        let instance_name = GoIdentifier::Public {
            name: &format!("{world_name}-instance"),
        };
        let new_factory = GoIdentifier::Public {
            name: &format!("new-{world_name}-factory"),
        };
        let instance = GoIdentifier::Public {
            name: &format!("{world_name}-instance"),
        };

        // Generate factory and instance struct types
        self.generate_types(&factory, &instance_name)?;

        // Generate factory constructor
        self.generate_factory_constructor(&factory, &new_factory)?;

        // Generate instance methods
        self.generate_instance_methods(&factory, &instance)?;

        Ok(())
    }

    /// Generate factory and instance struct definitions
    fn generate_types(
        &mut self,
        factory: &GoIdentifier,
        instance_name: &GoIdentifier,
    ) -> Result<()> {
        let go_imports = self.config.go_imports;

        quote_in! { self.context.out =>
            $['\n']
            type $factory struct {
                runtime $(&go_imports.wazero_runtime)
                module  $(&go_imports.wazero_compiled_module)
            }
            $['\n']
            type $instance_name struct {
                module $(&go_imports.wazero_api_module)
            }
            $['\n']
        };

        Ok(())
    }

    /// Generate the factory constructor function
    fn generate_factory_constructor(
        &mut self,
        factory: &GoIdentifier,
        new_factory: &GoIdentifier,
    ) -> Result<()> {
        let go_imports = self.config.go_imports;
        let wasm_var_name = self.config.wasm_var_name;

        // Build the parameter list
        let params = self.build_parameters()?;

        quote_in! { self.context.out =>
            func $new_factory(
                $['\r']
                $params
                $['\r']
            ) (*$factory, error) {
                wazeroRuntime := $(&go_imports.wazero_new_runtime)(ctx)

                $(for chain in self.config.import_chains.values() =>
                    $chain
                    $['\r']
                )

                module, err := wazeroRuntime.CompileModule(ctx, $wasm_var_name)
                if err != nil {
                    return nil, err
                }
                return &$factory{
                    runtime: wazeroRuntime,
                    module:  module,
                }, nil
            }
            $['\n']
        };

        Ok(())
    }

    /// Build parameter list for factory constructor
    fn build_parameters(&self) -> Result<Tokens<Go>> {
        let go_imports = self.config.go_imports;
        let world_name = self.config.world_name;
        let interface_params = &self.config.interface_params;

        Ok(quote! {
            ctx $(&go_imports.context),
            $(for interface_name in interface_params.iter() =>
            $(GoIdentifier::Local { name: interface_name }) $(GoIdentifier::Public { name: &format!("i-{world_name}-{interface_name}")}),)
        })
    }

    /// Generate instance methods (Instantiate, Close, etc.)
    fn generate_instance_methods(
        &mut self,
        factory: &GoIdentifier,
        instance: &GoIdentifier,
    ) -> Result<()> {
        let go_imports = self.config.go_imports;

        quote_in! { self.context.out =>
            func (f *$factory) Instantiate(ctx $(&go_imports.context)) (*$instance, error) {
                module, err := f.runtime.InstantiateModule(ctx, f.module, $(&go_imports.wazero_new_module_config)())
                if err != nil {
                    return nil, err
                }
                return &$instance{module: module}, nil
            }
            $['\n']
            func (f *$factory) Close(ctx $(&go_imports.context)) error {
                return f.runtime.Close(ctx)
            }
            $['\n']
            func (i *$instance) Close(ctx $(&go_imports.context)) error {
                return i.module.Close(ctx)
            }
        };

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_generation() {
        let mut context = GenerationContext::new();
        let go_imports = GoImports::new();
        let wasm_var_name = GoIdentifier::Private {
            name: "wasm-file-test",
        };

        let config = FactoryConfig {
            world_name: "test",
            go_imports: &go_imports,
            interface_params: vec!["logger".to_string()],
            import_chains: BTreeMap::new(),
            wasm_var_name: &wasm_var_name,
        };

        let generator = FactoryGenerator::new(&mut context, config);
        let result = generator.generate();

        assert!(result.is_ok());

        // Format the output to check the generated code
        let mut writer = genco::fmt::FmtWriter::new(String::new());
        let fmt =
            genco::fmt::Config::from_lang::<Go>().with_indentation(genco::fmt::Indentation::Tab);
        let config = genco::lang::go::Config::default();

        context
            .out
            .format_file(&mut writer.as_formatter(&fmt), &config)
            .unwrap();

        let output_str = writer.into_inner();

        // Should contain factory struct
        assert!(output_str.contains("type TestFactory struct"));

        // Should contain instance struct
        assert!(output_str.contains("type TestInstance struct"));

        // Should contain factory constructor
        assert!(output_str.contains("func NewTestFactory"));

        // Should contain Instantiate method
        assert!(output_str.contains("func (f *TestFactory) Instantiate"));
    }

    #[test]
    fn test_factory_with_interfaces() {
        let mut context = GenerationContext::new();
        let go_imports = GoImports::new();
        let wasm_var_name = GoIdentifier::Private {
            name: "wasm-file-basic",
        };

        // Create a sample import chain
        let mut import_chains = BTreeMap::new();
        import_chains.insert(
            "test-module".to_string(),
            quote! {
                _, err := wazeroRuntime.NewHostModuleBuilder("test-module").
                Instantiate(ctx)
                if err != nil {
                    return nil, err
                }
            },
        );

        let config = FactoryConfig {
            world_name: "basic",
            go_imports: &go_imports,
            interface_params: vec!["logger".to_string(), "storage".to_string()],
            import_chains,
            wasm_var_name: &wasm_var_name,
        };

        let generator = FactoryGenerator::new(&mut context, config);
        let result = generator.generate();

        assert!(result.is_ok());

        // Format the output to check the generated code
        let mut writer = genco::fmt::FmtWriter::new(String::new());
        let fmt =
            genco::fmt::Config::from_lang::<Go>().with_indentation(genco::fmt::Indentation::Tab);
        let config = genco::lang::go::Config::default();

        context
            .out
            .format_file(&mut writer.as_formatter(&fmt), &config)
            .unwrap();

        let output_str = writer.into_inner();

        // Should have interface parameters in constructor
        assert!(output_str.contains("logger IBasicLogger"));
        assert!(output_str.contains("storage IBasicStorage"));

        // Should include the import chain
        assert!(output_str.contains("NewHostModuleBuilder"));
    }
}
