use crate::context::GenerationContext;
use anyhow::Result;
use genco::lang::go::Import;
use genco::prelude::*;
use gravity_go::Go;
use heck::ToUpperCamelCase;
use std::collections::BTreeMap;
use wit_bindgen_core::wit_parser::{Function, InterfaceId, Resolve, WorldItem, WorldKey};

/// Struct to hold Go import references
pub struct GoImports {
    pub context: Import,
    pub errors: Import,
    pub wazero_runtime: Import,
    pub wazero_new_runtime: Import,
    pub wazero_new_module_config: Import,
    pub wazero_compiled_module: Import,
    pub wazero_api_module: Import,
    pub wazero_api_memory: Import,
}

impl GoImports {
    pub fn new() -> Self {
        Self {
            context: genco::lang::go::import("context", "Context"),
            errors: genco::lang::go::import("errors", "New"),
            wazero_runtime: genco::lang::go::import("github.com/tetratelabs/wazero", "Runtime"),
            wazero_new_runtime: genco::lang::go::import(
                "github.com/tetratelabs/wazero",
                "NewRuntime",
            ),
            wazero_new_module_config: genco::lang::go::import(
                "github.com/tetratelabs/wazero",
                "NewModuleConfig",
            ),
            wazero_compiled_module: genco::lang::go::import(
                "github.com/tetratelabs/wazero",
                "CompiledModule",
            ),
            wazero_api_module: genco::lang::go::import(
                "github.com/tetratelabs/wazero/api",
                "Module",
            ),
            wazero_api_memory: genco::lang::go::import(
                "github.com/tetratelabs/wazero/api",
                "Memory",
            ),
        }
    }
}

/// Result of import generation
pub struct ImportResult {
    /// Interface names that should be parameters to the factory constructor
    pub interface_params: Vec<String>,
    /// Import chains for host module builders (grouped by module name)
    pub import_chains: BTreeMap<String, Tokens<Go>>,
}

/// Generator for imports (interfaces, types, functions)
struct ImportGenerator<'a> {
    context: &'a mut GenerationContext,
    resolve: &'a Resolve,
    world_name: String,
    go_imports: &'a GoImports,
    interface_params: Vec<String>,
    import_chains: BTreeMap<String, Tokens<Go>>,
}

impl<'a> ImportGenerator<'a> {
    fn new(
        context: &'a mut GenerationContext,
        resolve: &'a Resolve,
        world_name: &str,
        go_imports: &'a GoImports,
    ) -> Self {
        Self {
            context,
            resolve,
            world_name: world_name.to_string(),
            go_imports,
            interface_params: Vec::new(),
            import_chains: BTreeMap::new(),
        }
    }

    /// Main entry point - generates all imports for a world
    fn generate(
        &mut self,
        world_imports: &indexmap::IndexMap<WorldKey, WorldItem>,
    ) -> Result<ImportResult> {
        // Process each import
        for (_import_name, world_item) in world_imports.iter() {
            match world_item {
                WorldItem::Interface { id, .. } => {
                    self.process_interface(*id)?;
                }
                WorldItem::Type(type_id) => {
                    self.process_type(*type_id)?;
                }
                WorldItem::Function(_func) => {
                    // TODO: Handle standalone function imports
                }
            }
        }

        Ok(ImportResult {
            interface_params: self.interface_params.clone(),
            import_chains: self.import_chains.clone(),
        })
    }

    /// Process an interface import
    fn process_interface(&mut self, interface_id: InterfaceId) -> Result<()> {
        let interface = &self.resolve.interfaces[interface_id];
        let interface_name = interface
            .name
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Interface missing name"))?;

        // Add to interface parameters for factory constructor
        self.interface_params.push(interface_name.clone());

        // Generate interface type definition
        self.generate_interface_type(interface_id, interface_name)?;

        // Define types used by this interface
        for type_id in interface.types.values() {
            self.define_type(*type_id)?;
        }

        // Generate host module builder
        self.generate_host_module_builder(interface_id, interface_name)?;

        Ok(())
    }

    /// Generate the Go interface type definition
    fn generate_interface_type(
        &mut self,
        interface_id: InterfaceId,
        interface_name: &str,
    ) -> Result<()> {
        let interface = &self.resolve.interfaces[interface_id];

        // Generate interface name like IBasicLogger
        let interface_type_name = format!(
            "I{}{}",
            self.world_name.to_upper_camel_case(),
            interface_name.to_upper_camel_case()
        );

        // Build method signatures
        let mut methods = Tokens::new();
        for func in interface.functions.values() {
            self.generate_interface_method(func, &mut methods)?;
        }

        // Generate the interface definition
        quote_in! { self.context.out =>
            $['\n']
            type $(interface_type_name) interface {
                $methods
            }
        };

        Ok(())
    }

    /// Generate a single method signature for an interface
    fn generate_interface_method(&self, func: &Function, methods: &mut Tokens<Go>) -> Result<()> {
        let method_name = func.name.to_upper_camel_case();

        // Build parameters
        let mut params = vec![quote! { ctx $(self.go_imports.context.clone()) }];
        for (param_name, param_type) in &func.params {
            let go_type = crate::resolve_type(param_type, self.resolve)?;
            params.push(quote! { $param_name $go_type });
        }

        // Build result type
        let result = if let Some(result_type) = &func.result {
            let go_type = crate::resolve_type(result_type, self.resolve)?;
            quote! { $go_type }
        } else {
            quote! {}
        };

        // Generate method
        quote_in! { *methods =>
            $['\r']$(method_name)(
                $(for param in params join (,$['\r']) => $param),
            )$(if !result.is_empty() => $[' ']$result)
        };

        Ok(())
    }

    /// Generate host module builder for an interface
    fn generate_host_module_builder(
        &mut self,
        interface_id: InterfaceId,
        interface_name: &str,
    ) -> Result<()> {
        let interface = &self.resolve.interfaces[interface_id];

        // Build module name
        let import_module_name = if let Some(package_id) = interface.package {
            let package = &self.resolve.packages[package_id];
            format!(
                "{}:{}/{}",
                package.name.namespace, package.name.name, interface_name
            )
        } else {
            interface_name.to_string()
        };

        // Get or create import chain for this module
        self.import_chains
            .entry(import_module_name.clone())
            .or_insert_with(|| {
                quote! {
                    _, err := wazeroRuntime.NewHostModuleBuilder($(quoted(&import_module_name))).
                }
            });

        // Generate function builders for each function
        for func in interface.functions.values() {
            let mut func_builder = Tokens::<Go>::new();
            self.generate_function_builder_content(func, interface_name, &mut func_builder)?;

            // Append to import chain
            if let Some(chain) = self.import_chains.get_mut(&import_module_name) {
                quote_in! { *chain =>
                    $func_builder
                };
            }
        }

        // Add instantiate call
        let import_chain = self.import_chains.get_mut(&import_module_name).unwrap();
        quote_in! { *import_chain =>
            $['\r']Instantiate(ctx)
            $['\r']if err != nil {
                return nil, err
            }
        };

        Ok(())
    }

    /// Generate a function builder for a host module
    fn generate_function_builder_content(
        &self,
        func: &Function,
        interface_name: &str,
        output: &mut Tokens<Go>,
    ) -> Result<()> {
        // For now, generate a simple stub
        // TODO: Use wit_bindgen_core::abi::call to generate proper function body

        let func_name = &func.name;
        let param_name = interface_name; // This will be the interface parameter name

        // Generate simple string handling for basic cases
        // This is a simplified version - the real implementation needs wit_bindgen_core::abi::call
        quote_in! { *output =>
            $['\r']NewFunctionBuilder().
            $['\r']WithFunc(func(
                ctx $(self.go_imports.context.clone()),
                mod $(self.go_imports.wazero_api_module.clone()),
                arg0 uint32,
                arg1 uint32,
            ) {
                buf, ok := mod.Memory().Read(arg0, arg1)
                if !ok {
                    panic($(self.go_imports.errors.clone())("failed to read bytes from memory"))
                }
                str := string(buf)
                $(param_name).$(func_name.to_upper_camel_case())(ctx, str)
            }).
            $['\r']Export($(quoted(func_name))).
        };

        Ok(())
    }

    /// Process a standalone type import
    fn process_type(&mut self, type_id: wit_bindgen_core::wit_parser::TypeId) -> Result<()> {
        self.define_type(type_id)?;
        Ok(())
    }

    /// Define a type in the output
    fn define_type(&mut self, type_id: wit_bindgen_core::wit_parser::TypeId) -> Result<()> {
        let type_def = self.resolve.types.get(type_id).unwrap();

        // For now, just generate a comment
        // TODO: Implement full type generation
        if let Some(_name) = &type_def.name {
            quote_in! { self.context.out =>
                $['\n']
                // Type: $(name)
                // TODO: Generate type definition
            };
        }

        Ok(())
    }
}

/// Generate imports for a WebAssembly component world.
///
/// This function generates:
/// - Go interface definitions for imported interfaces
/// - Host module builder chains for wazero runtime
///
/// Returns an `ImportResult` containing:
/// - `interface_params`: Names of interfaces that should be parameters to the factory constructor
/// - `import_chains`: Host module builder code for each imported module
pub fn generate_imports_with_chains(
    context: &mut GenerationContext,
    resolve: &Resolve,
    world_name: &str,
    world_imports: &indexmap::IndexMap<WorldKey, WorldItem>,
    go_imports: &GoImports,
) -> Result<ImportResult> {
    let mut generator = ImportGenerator::new(context, resolve, world_name, go_imports);
    generator.generate(world_imports)
}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::indexmap;
    use wit_bindgen_core::wit_parser::{
        Function, FunctionKind, Interface, Package, PackageName, Type, World, WorldId,
    };

    fn create_test_world_with_interface() -> (Resolve, WorldId) {
        let mut resolve = Resolve::default();

        // Create a package
        let package_name = PackageName {
            namespace: "test".to_string(),
            name: "pkg".to_string(),
            version: None,
        };
        let package_id = resolve.packages.alloc(Package {
            name: package_name.clone(),
            interfaces: Default::default(),
            worlds: Default::default(),
            docs: Default::default(),
        });

        // Create an interface with a function
        let interface_id = resolve.interfaces.alloc(Interface {
            name: Some("logger".to_string()),
            package: Some(package_id),
            functions: indexmap! {
                "log".to_string() => Function {
                    name: "log".to_string(),
                    params: vec![("message".to_string(), Type::String)],
                    result: None,
                    kind: FunctionKind::Freestanding,
                    docs: Default::default(),
                    stability: Default::default(),
                }
            },
            types: Default::default(),
            docs: Default::default(),
            stability: Default::default(),
        });

        // Create a world with the interface as import
        let world = World {
            name: "test-world".to_string(),
            imports: indexmap! {
                WorldKey::Name("logger".to_string()) => WorldItem::Interface { id: interface_id, stability: Default::default() }
            },
            exports: Default::default(),
            docs: Default::default(),
            stability: Default::default(),
            package: Some(package_id),
            includes: Default::default(),
            include_names: Default::default(),
        };

        let world_id = resolve.worlds.alloc(world);
        (resolve, world_id)
    }

    #[test]
    fn test_generate_imports_with_interface() {
        let (resolve, world_id) = create_test_world_with_interface();
        let world = &resolve.worlds[world_id];
        let mut context = GenerationContext::new();
        let go_imports = GoImports::new();

        let result = generate_imports_with_chains(
            &mut context,
            &resolve,
            &world.name,
            &world.imports,
            &go_imports,
        )
        .unwrap();
        let interface_params = result.interface_params;

        // Check that we got the interface parameter
        assert_eq!(interface_params.len(), 1);
        assert_eq!(interface_params[0], "logger");

        // Check that interface type was generated
        let output = context.out.to_string().unwrap();
        assert!(output.contains("type ITestWorldLogger interface"));
    }
}
