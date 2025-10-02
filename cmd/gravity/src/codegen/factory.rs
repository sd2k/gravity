use std::collections::BTreeMap;

use genco::prelude::*;

use crate::{
    codegen::ir::AnalyzedImports,
    go::{
        GoIdentifier, comment,
        imports::{
            CONTEXT_CONTEXT, ERRORS_NEW, SYNC_MUTEX, WAZERO_API_MEMORY, WAZERO_API_MODULE,
            WAZERO_COMPILED_MODULE, WAZERO_NEW_MODULE_CONFIG, WAZERO_NEW_RUNTIME, WAZERO_RUNTIME,
        },
    },
};

/// Configuration for factory generation
pub struct FactoryConfig<'a> {
    pub analyzed_imports: &'a AnalyzedImports,
    pub import_chains: BTreeMap<String, Tokens<Go>>,
    pub wasm_var_name: &'a GoIdentifier,
}

/// Generator for factory and instance types
pub struct FactoryGenerator<'a> {
    config: FactoryConfig<'a>,
}

impl<'a> FactoryGenerator<'a> {
    /// Create a new factory generator with the given config.
    pub fn new(config: FactoryConfig<'a>) -> Self {
        Self { config }
    }

    /// Get the instance name from the analyzed imports.
    pub fn instance_name(&self) -> &GoIdentifier {
        &self.config.analyzed_imports.instance_name
    }

    /// Generate the `writeString` helper function.
    fn generate_write_string(&self, tokens: &mut Tokens<Go>) {
        // Add writeString helper function for interface string returns
        quote_in! { *tokens =>
            $(comment(&[
                "writeString will put a Go string into the Wasm memory following the Component",
                "Model calling conventions, such as allocating memory with the realloc function",
            ]))
            func writeString(
                ctx $CONTEXT_CONTEXT,
                s string,
                memory $WAZERO_API_MEMORY,
                realloc api.Function,
            ) (uint64, uint64, error) {
                if len(s) == 0 {
                    return 1, 0, nil
                }

                results, err := realloc.Call(ctx, 0, 0, 1, uint64(len(s)))
                if err != nil {
                    return 1, 0, err
                }
                ptr := results[0]
                ok := memory.Write(uint32(ptr), []byte(s))
                if !ok {
                    return 1, 0, $ERRORS_NEW("failed to write string to wasm memory")
                }
                return uint64(ptr), uint64(len(s)), nil
            }
            $['\n']
        };
    }

    /// Generate the Factory struct, constructor, and methods.
    fn generate_factory(&self, tokens: &mut Tokens<Go>) {
        let AnalyzedImports {
            factory_name,
            instance_name,
            constructor_name,
            ..
        } = &self.config.analyzed_imports;
        let wasm_var_name = self.config.wasm_var_name;

        // Get type parameters as structured data
        let type_param_data = self.build_parameters_with_generics();

        // Collect resource info before quote_in! to avoid borrow issues
        let resource_info = self.collect_resource_info();

        // Build interface type parameters
        let mut interface_type_params: std::collections::HashMap<
            String,
            Vec<(GoIdentifier, GoIdentifier)>,
        > = std::collections::HashMap::new();
        for (iface_name, _resource_name, prefixed_name, type_param_name) in &resource_info {
            let value_type_param = GoIdentifier::public(format!("t-{}-value", prefixed_name));
            let pointer_type_param =
                GoIdentifier::public(format!("p-{}", String::from(type_param_name)));
            interface_type_params
                .entry(iface_name.clone())
                .or_insert_with(Vec::new)
                .push((value_type_param, pointer_type_param));
        }

        // Generate resource table type if we have resources
        let has_resources = !type_param_data.is_empty();

        if has_resources {
            self.generate_resource_table_type(tokens);
        }

        if has_resources {
            let interfaces = &self.config.analyzed_imports.interfaces;

            // Pre-build interface parameter tokens completely before quote_in
            let mut interface_params = Vec::new();
            for interface in interfaces.iter() {
                let interface_name = interface.name.split('/').last().unwrap_or(&interface.name);
                if let Some(iface_type_params) = interface_type_params.get(interface_name) {
                    let type_args: Vec<_> = iface_type_params
                        .iter()
                        .flat_map(|(v, p)| vec![v, p])
                        .collect();
                    let param_tokens = quote!($(&interface.constructor_param_name) $(&interface.go_interface_name)[$(for tp in type_args join (, ) => $tp)],);
                    interface_params.push(param_tokens);
                } else {
                    let param_tokens = quote!($(&interface.constructor_param_name) $(&interface.go_interface_name),);
                    interface_params.push(param_tokens);
                }
            }

            quote_in! { *tokens =>
                $['\n']
                type $factory_name[$(for (value_param, pointer_param, pointer_iface) in &type_param_data join (, ) => $value_param any, $pointer_param $pointer_iface[$value_param])] struct {
                    runtime $WAZERO_RUNTIME
                    module  $WAZERO_COMPILED_MODULE
                    $['\r']
                    $(for (_iface_name, _resource_name, prefixed_name, type_param_name) in resource_info.iter() join ($['\r']) =>
                    $(&GoIdentifier::public(format!("{}-resource-table", prefixed_name))) *$(&GoIdentifier::private(format!("{}-resource-table", prefixed_name)))[$(&GoIdentifier::public(format!("t-{}-value", prefixed_name))), $(&GoIdentifier::public(format!("p-{}", String::from(type_param_name))))])
                }
                $['\n']
                func $constructor_name[$(for (value_param, pointer_param, pointer_iface) in &type_param_data join (, ) => $value_param any, $pointer_param $pointer_iface[$value_param])](
                    $['\r']
                    ctx $CONTEXT_CONTEXT,
                    $(for param_tokens in &interface_params join ($['\r']) => $param_tokens)
                    $['\r']
                ) (*$factory_name[$(for (value_param, pointer_param, _) in &type_param_data join (, ) => $value_param, $pointer_param)], error) {
                    $['\r']
                    wazeroRuntime := $WAZERO_NEW_RUNTIME(ctx)
                    $['\r']
                    $(comment(&["Initialize resource tables before host module instantiation"]))
                    $(for (_iface_name, _resource_name, prefixed_name, type_param_name) in resource_info.iter() join ($['\r']) =>
                    $(&GoIdentifier::private(format!("{}_resource_table", prefixed_name))) := new$(&GoIdentifier::public(format!("{}-resource-table", prefixed_name)))[$(&GoIdentifier::public(format!("t-{}-value", prefixed_name))), $(&GoIdentifier::public(format!("p-{}", String::from(type_param_name))))]())
                    $['\r']
                    $['\r']
                    $(comment(&["Instantiate import host modules"]))
                    $(for chain in self.config.import_chains.values() =>
                    $chain
                    $['\r']
                )
                    $['\r']
                    $(comment(&["Instantiate export resource management host modules"]))
                    $(for chain in self.generate_export_resource_chains().values() =>
                    $chain
                    $['\r']
                )
                    $['\r']
                    $(comment(&[
                    "Compiling the module takes a LONG time, so we want to do it once and hold",
                       "onto it with the Runtime",
                ]))
                    module, err := wazeroRuntime.CompileModule(ctx, $wasm_var_name)
                    if err != nil {
                        return nil, err
                    }
                    return &$factory_name[$(for (value_param, pointer_param, _) in &type_param_data join (, ) => $value_param, $pointer_param)]{
                        runtime: wazeroRuntime,
                        module:  module,
                        $['\r']
                        $(for (_iface_name, _resource_name, prefixed_name, _) in resource_info.iter() =>
                        $(&GoIdentifier::public(format!("{}-resource-table", prefixed_name))): $(&GoIdentifier::private(format!("{}_resource_table", prefixed_name))),$['\r'])
                    }, nil
                }
            $['\n']
            func (f *$factory_name[$(for (value_param, pointer_param, _) in &type_param_data join (, ) => $value_param, $pointer_param)]) Instantiate(ctx $CONTEXT_CONTEXT) (*$instance_name[$(for (value_param, pointer_param, _) in &type_param_data join (, ) => $value_param, $pointer_param)], error) {
                if module, err := f.runtime.InstantiateModule(ctx, f.module, $WAZERO_NEW_MODULE_CONFIG()); err != nil {
                    return nil, err
                } else {
                    return &$instance_name[$(for (value_param, pointer_param, _) in &type_param_data join (, ) => $value_param, $pointer_param)]{
                        module: module,
                        $['\r']
                        $(for (_iface_name, _resource_name, prefixed_name, _) in resource_info.iter() =>
                        $(&GoIdentifier::public(format!("{}-resource-table", prefixed_name))): f.$(&GoIdentifier::public(format!("{}-resource-table", prefixed_name))),$['\r'])
                    }, nil
                }
            }
            $['\n']
            func (f *$factory_name[$(for (value_param, pointer_param, _) in &type_param_data join (, ) => $value_param, $pointer_param)]) Close(ctx $CONTEXT_CONTEXT) {
                f.runtime.Close(ctx)
            }
            $['\n']
            };
        } else {
            let interfaces = &self.config.analyzed_imports.interfaces;
            quote_in! { *tokens =>
                $['\n']
                type $factory_name struct {
                    runtime $WAZERO_RUNTIME
                    module  $WAZERO_COMPILED_MODULE
                }
                $['\n']
                func $constructor_name(
                    ctx $CONTEXT_CONTEXT,
                    $(for interface in interfaces.iter() join ($['\r']) =>
                    $(&interface.constructor_param_name) $(&interface.go_interface_name),)
                ) (*$factory_name, error) {
                    wazeroRuntime := $WAZERO_NEW_RUNTIME(ctx)

                    $(comment(&["Instantiate import host modules"]))
                    $(for chain in self.config.import_chains.values() =>
                        $chain
                        $['\r']
                    )

                    $(comment(&[
                        "Compiling the module takes a LONG time, so we want to do it once and hold",
                           "onto it with the Runtime",
                    ]))
                    module, err := wazeroRuntime.CompileModule(ctx, $wasm_var_name)
                    if err != nil {
                        return nil, err
                    }
                    return &$factory_name{
                        runtime: wazeroRuntime,
                        module:  module,
                    }, nil
                }
                $['\n']
                func (f *$factory_name) Instantiate(ctx $CONTEXT_CONTEXT) (*$instance_name, error) {
                    if module, err := f.runtime.InstantiateModule(ctx, f.module, $WAZERO_NEW_MODULE_CONFIG()); err != nil {
                        return nil, err
                    } else {
                        return &$instance_name{module}, nil
                    }
                }
                $['\n']
                func (f *$factory_name) Close(ctx $CONTEXT_CONTEXT) {
                    f.runtime.Close(ctx)
                }
                $['\n']
            };
        }
    }

    /// Generate the Instance struct, and methods.
    fn generate_instance(&self, tokens: &mut Tokens<Go>) {
        let instance_name = &self.config.analyzed_imports.instance_name;

        // Get type parameter data for resources
        let type_param_data = self.build_parameters_with_generics();
        let resource_info = self.collect_resource_info();
        let has_resources = !type_param_data.is_empty();

        if has_resources {
            // Generic instance with resource tables
            quote_in! { *tokens =>
                type $instance_name[$(for (value_param, pointer_param, pointer_iface) in &type_param_data join (, ) => $value_param any, $pointer_param $pointer_iface[$value_param])] struct {
                    module $WAZERO_API_MODULE
                    $['\r']
                    $(for (_iface_name, _resource_name, prefixed_name, type_param_name) in resource_info.iter() join ($['\r']) =>
                    $(&GoIdentifier::public(format!("{}-resource-table", prefixed_name))) *$(&GoIdentifier::private(format!("{}-resource-table", prefixed_name)))[$(&GoIdentifier::public(format!("t-{}-value", prefixed_name))), $(&GoIdentifier::public(format!("p-{}", String::from(type_param_name))))])
                }
                $['\n']
                func (i *$instance_name[$(for (value_param, pointer_param, _) in &type_param_data join (, ) => $value_param, $pointer_param)]) Close(ctx $CONTEXT_CONTEXT) error {
                    if err := i.module.Close(ctx); err != nil {
                        return err
                    }

                    return nil
                }
                $['\n']
            };
        } else {
            // Non-generic instance (no resources)
            quote_in! { *tokens =>
                type $instance_name struct {
                    module $WAZERO_API_MODULE
                }
                $['\n']
                func (i *$instance_name) Close(ctx $CONTEXT_CONTEXT) error {
                    if err := i.module.Close(ctx); err != nil {
                        return err
                    }

                    return nil
                }
                $['\n']
            };
        }
    }

    /// Build type parameters and parameter list for factory constructor
    fn build_parameters_with_generics(&self) -> Vec<(GoIdentifier, GoIdentifier, GoIdentifier)> {
        // Use collect_resource_info for consistency
        let resource_info = self.collect_resource_info();
        let mut result = Vec::new();

        for (_interface_name, _resource_name, prefixed_name, type_param_name) in &resource_info {
            let pointer_interface_name = GoIdentifier::public(format!("p-{}", prefixed_name));
            let value_type_param = GoIdentifier::public(format!("t-{}-value", prefixed_name));
            let pointer_type_param =
                GoIdentifier::public(format!("p-{}", String::from(type_param_name)));

            result.push((value_type_param, pointer_type_param, pointer_interface_name));
        }

        result
    }

    /// Collect resource information (interface name, resource name, prefixed name, and type parameter)
    fn collect_resource_info(&self) -> Vec<(String, String, String, GoIdentifier)> {
        let mut resources = Vec::new();
        let interfaces = &self.config.analyzed_imports.interfaces;

        for interface in interfaces.iter() {
            let interface_name = interface.name.split('/').last().unwrap_or(&interface.name);
            for method in &interface.methods {
                if method.name.contains("[constructor]") {
                    if let Some(ret) = &method.return_type {
                        if let crate::go::GoType::OwnHandle(name)
                        | crate::go::GoType::BorrowHandle(name)
                        | crate::go::GoType::Resource(name) = &ret.go_type
                        {
                            let prefixed_name = format!("{}-{}", interface_name, name);
                            let type_param_name =
                                GoIdentifier::public(format!("t-{}", prefixed_name));
                            resources.push((
                                interface_name.to_string(),
                                name.clone(),
                                prefixed_name,
                                type_param_name,
                            ));
                        }
                    }
                }
            }
        }

        resources
    }

    /// Generate export resource management host functions
    /// These are [resource-new] and [resource-drop] functions that WASM calls
    /// when it creates/destroys exported resources
    fn generate_export_resource_chains(&self) -> BTreeMap<String, Tokens<Go>> {
        let mut chains = BTreeMap::new();

        // Group exported resources by their wazero module name
        let mut resources_by_module: BTreeMap<
            String,
            Vec<&crate::codegen::ir::ExportedResourceInfo>,
        > = BTreeMap::new();

        for resource in &self.config.analyzed_imports.exported_resources {
            resources_by_module
                .entry(resource.wazero_export_module_name.clone())
                .or_insert_with(Vec::new)
                .push(resource);
        }

        // Generate a host module for each export interface
        for (i, (module_name, resources)) in resources_by_module.into_iter().enumerate() {
            let err = &GoIdentifier::private(format!("err_export{i}"));
            let mut chain = quote! {
                _, $err := wazeroRuntime.NewHostModuleBuilder($(quoted(&module_name))).
            };

            for resource in resources {
                let resource_name = &resource.resource_name;

                // Generate [resource-new]foo function
                // For MVP: simple identity mapping (rep -> rep)
                chain.push();
                quote_in! { chain =>
                    NewFunctionBuilder().
                    WithFunc(func(
                        ctx $CONTEXT_CONTEXT,
                        mod $WAZERO_API_MODULE,
                        rep uint32,
                    ) uint32 {
                        $(comment(&[
                            &format!("[resource-new]{}: allocate handle for WASM-created resource", resource_name),
                            "For MVP: using identity mapping (handle = rep)",
                        ]))
                        return rep
                    }).
                    Export($(quoted(&format!("[resource-new]{}", resource_name)))).
                };

                // Generate [resource-drop]foo function
                chain.push();
                quote_in! { chain =>
                    NewFunctionBuilder().
                    WithFunc(func(
                        ctx $CONTEXT_CONTEXT,
                        mod $WAZERO_API_MODULE,
                        handle uint32,
                    ) {
                        $(comment(&[
                            &format!("[resource-drop]{}: cleanup for WASM resource", resource_name),
                            "For MVP: no-op (WASM manages its own resources)",
                        ]))
                        _ = handle
                    }).
                    Export($(quoted(&format!("[resource-drop]{}", resource_name)))).
                };
            }

            chain.push();
            quote_in! { chain =>
                Instantiate(ctx)
                if $err != nil {
                    return nil, $err
                }
            };

            chains.insert(module_name, chain);
        }

        chains
    }

    /// Generate the resource table type
    fn generate_resource_table_type(&self, tokens: &mut Tokens<Go>) {
        // Generate a separate table type for each resource
        for (interface_name, resource_name, prefixed_name, type_param_name) in
            self.collect_resource_info()
        {
            let table_type_name =
                GoIdentifier::private(format!("{}-resource-table", prefixed_name));
            let resource_interface = GoIdentifier::public(&prefixed_name);
            let handle_type = GoIdentifier::private(format!("{}-handle", prefixed_name));
            let value_type_param = GoIdentifier::public(format!("t-{}-value", prefixed_name));
            let pointer_type_param =
                GoIdentifier::public(format!("p-{}", String::from(&type_param_name)));
            let pointer_interface_name = GoIdentifier::public(format!("p-{}", prefixed_name));

            let pointer_comment = format!(
                "{} constrains a pointer to a type implementing the {} interface.",
                String::from(&pointer_interface_name),
                String::from(&resource_interface)
            );

            let table_comment = format!(
                "{} is a resource table for {} resources from the {} interface.",
                String::from(&table_type_name),
                resource_name,
                interface_name
            );

            quote_in! { *tokens =>
                $['\n']
                $(comment(&[pointer_comment.as_str()]))
                type $(&pointer_interface_name)[$(&value_type_param) any] interface {
                    *$(&value_type_param)
                    $(&resource_interface)
                }
                $['\n']
                $(comment(&[table_comment.as_str()]))
                type $(&table_type_name)[$(&value_type_param) any, $(&pointer_type_param) $(&pointer_interface_name)[$(&value_type_param)]] struct {
                    mu $SYNC_MUTEX
                    nextHandle uint32
                    table map[$(&handle_type)]*$(&value_type_param)
                }
                $['\n']
                func new$(&GoIdentifier::public(format!("{}-resource-table", prefixed_name)))[$(&value_type_param) any, $(&pointer_type_param) $(&pointer_interface_name)[$(&value_type_param)]]() *$(&table_type_name)[$(&value_type_param), $(&pointer_type_param)] {
                    return &$(&table_type_name)[$(&value_type_param), $(&pointer_type_param)]{
                        nextHandle: 1,
                        table: make(map[$(&handle_type)]*$(&value_type_param)),
                    }
                }
                $['\n']
                $(comment(&["Store adds a resource to the table and returns its handle."]))
                func (t *$(&table_type_name)[$(&value_type_param), $(&pointer_type_param)]) Store(resource $(&value_type_param)) $(&handle_type) {
                    t.mu.Lock()
                    defer t.mu.Unlock()
                    handle := $(&handle_type)(t.nextHandle)
                    t.nextHandle++
                    t.table[handle] = &resource
                    return handle
                }
                $['\n']
                $(comment(&["get returns a pointer to the resource from the table by its handle."]))
                func (t *$(&table_type_name)[$(&value_type_param), $(&pointer_type_param)]) get(handle $(&handle_type)) ($(&pointer_type_param), bool) {
                    t.mu.Lock()
                    defer t.mu.Unlock()
                    resource, ok := t.table[handle]
                    if !ok {
                        var zero $(&pointer_type_param)
                        return zero, false
                    }
                    return resource, true
                }
                $['\n']
                $(comment(&["Get retrieves a resource from the table by its handle."]))
                func (t *$(&table_type_name)[$(&value_type_param), $(&pointer_type_param)]) Get(handle $(&handle_type)) ($(&value_type_param), bool) {
                    t.mu.Lock()
                    defer t.mu.Unlock()
                    resource, ok := t.table[handle]
                    if !ok {
                        var zero $(&value_type_param)
                        return zero, false
                    }
                    return *resource, true
                }
                $['\n']
                $(comment(&["Remove deletes a resource from the table."]))
                func (t *$(&table_type_name)[$(&value_type_param), $(&pointer_type_param)]) Remove(handle $(&handle_type)) {
                    t.mu.Lock()
                    defer t.mu.Unlock()
                    delete(t.table, handle)
                }
                $['\n']
            }
        }
    }
}

impl<'a> FormatInto<Go> for &FactoryGenerator<'a> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        self.generate_factory(tokens);
        tokens.push();
        self.generate_instance(tokens);
        tokens.push();
        self.generate_write_string(tokens);
        tokens.push();
    }
}

#[cfg(test)]
mod tests {
    use genco::lang::go::Tokens;

    use crate::{
        codegen::{FactoryGenerator, factory::FactoryConfig, ir::AnalyzedImports},
        go::GoIdentifier,
    };

    #[test]
    fn test_generate_write_string() {
        let analyzed_imports = &AnalyzedImports {
            interfaces: vec![],
            standalone_types: vec![],
            standalone_functions: vec![],
            exported_resources: vec![],
            factory_name: GoIdentifier::public("test-factory"),
            instance_name: GoIdentifier::public("test-instance"),
            constructor_name: GoIdentifier::public("test-constructor"),
        };
        let config = FactoryConfig {
            analyzed_imports,
            import_chains: Default::default(),
            wasm_var_name: &GoIdentifier::public("test-wasm"),
        };
        let generator = FactoryGenerator::new(config);
        let mut tokens = Tokens::new();
        generator.generate_write_string(&mut tokens);

        assert!(tokens.to_string().unwrap().contains("func writeString"));
    }
}
