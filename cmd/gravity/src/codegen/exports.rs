use genco::prelude::*;
use wit_bindgen_core::wit_parser::{
    Function, FunctionKind, Resolve, SizeAlign, TypeDefKind, World, WorldItem,
};

use crate::codegen::ir::AnalyzedImports;
use crate::go::{GoIdentifier, GoImports, GoResult, GoType};

pub struct ExportConfig<'a> {
    pub instance: &'a GoIdentifier,
    pub go_imports: &'a GoImports,
    pub world: &'a World,
    pub resolve: &'a Resolve,
    pub sizes: &'a SizeAlign,
    pub analyzed_imports: &'a AnalyzedImports,
}

pub struct ExportGenerator<'a> {
    config: ExportConfig<'a>,
}

impl<'a> ExportGenerator<'a> {
    pub fn new(config: ExportConfig<'a>) -> Self {
        Self { config }
    }

    /// Generate the Go function code for the given function.
    ///
    /// The signature is obtained by:
    /// - getting the function parameters from the `wit_parser::Function`, converting
    ///   names to to Go identifiers and types to Go types.
    /// - similar for the result
    ///
    /// To implement the body, we:
    /// - creating a `Func` struct which implements `Bindgen` and passing it to the
    ///   `wit_bindgen_core::abi::call` function. This will call `Func::emit` lots of
    ///   times, one for each instruction in the function, and `Func::emit` will generate
    ///   Go code for each instruction
    fn generate_function(
        &self,
        func: &Function,
        interface_name: &str,
        is_interface_export: bool,
        tokens: &mut Tokens<Go>,
    ) {
        use wit_bindgen_core::wit_parser::{Handle, Type, TypeDefKind};

        // Validate: we only support borrow parameters, not owned
        // This simplifies lifecycle management dramatically
        for (param_name, wit_type) in &func.params {
            if let Type::Id(type_id) = wit_type {
                if let Some(type_def) = self.config.resolve.types.get(*type_id) {
                    if let TypeDefKind::Handle(Handle::Own(_)) = type_def.kind {
                        panic!(
                            "Function '{}' has owned resource parameter '{}'. \n\
                            Gravity only supports borrow<T> parameters in exports to simplify resource lifecycle management.\n\
                            Owned parameters would require complex state tracking (generation counters, state machines, etc.)\n\
                            \n\
                            To fix:\n\
                            - Change 'func(resource: foo)' to 'func(resource: borrow<foo>)'\n\
                            - Owned returns (func() -> foo) are still supported!\n\
                            \n\
                            This design keeps host resource management simple and explicit while still allowing\n\
                            guests to create and return resources to the host.",
                            func.name, param_name
                        );
                    }
                }
            }
        }

        let params = func
            .params
            .iter()
            .map(|(name, wit_type)| {
                let resolved = crate::resolve_type(wit_type, self.config.resolve);
                let prefixed = self.resolve_type_with_interface(&resolved, interface_name);
                match prefixed {
                    GoType::ValueOrOk(t) => (GoIdentifier::local(name), *t),
                    t => (GoIdentifier::local(name), t),
                }
            })
            .collect::<Vec<_>>();

        let result = if let Some(wit_type) = &func.result {
            let resolved = crate::resolve_type(wit_type, self.config.resolve);
            GoResult::Anon(self.resolve_type_with_interface(&resolved, interface_name))
        } else {
            GoResult::Empty
        };

        // Detect if function has resource parameters or returns
        let resource_info = self.detect_resource_in_function(func);

        let mut f = if let Some((res_name, _)) = &resource_info {
            crate::Func::export_with_resource(
                result,
                self.config.sizes,
                self.config.go_imports,
                interface_name.to_string(),
                res_name.clone(),
            )
        } else {
            crate::Func::export(result, self.config.sizes, self.config.go_imports)
        };
        // Build the full qualified export name for the wasm function call
        let qualified_name = if is_interface_export && !interface_name.is_empty() {
            // Get the full interface name with package
            let full_interface_name = self
                .config
                .world
                .exports
                .iter()
                .find_map(|(key, _)| {
                    if let wit_bindgen_core::wit_parser::WorldKey::Interface(id) = key {
                        let iface = &self.config.resolve.interfaces[*id];
                        if let Some(name) = &iface.name {
                            let short_name = name.split('/').last().unwrap_or(name);
                            if short_name == interface_name {
                                if let Some(package_id) = iface.package {
                                    let package = &self.config.resolve.packages[package_id];
                                    return Some(format!(
                                        "{}:{}/{}",
                                        package.name.namespace, package.name.name, name
                                    ));
                                }
                            }
                        }
                    }
                    None
                })
                .unwrap_or_else(|| interface_name.to_string());
            format!("{}#{}", full_interface_name, func.name)
        } else {
            func.name.clone()
        };

        // Set the qualified export name before generating the body
        // This avoids string replacement which breaks genco's import tracking
        f.set_export_name(qualified_name);

        wit_bindgen_core::abi::call(
            self.config.resolve,
            wit_bindgen_core::abi::AbiVariant::GuestExport,
            wit_bindgen_core::abi::LiftLower::LowerArgsLiftResults,
            func,
            &mut f,
            // async is not currently supported
            false,
        );

        let arg_assignments = f
            .args()
            .iter()
            .zip(&params)
            .map(|(arg, (param, _))| (arg, param))
            .collect::<Vec<_>>();
        let fn_name = &GoIdentifier::public(&func.name);

        // Collect resource type parameters for instance receiver
        let mut type_params = Vec::new();
        for interface in &self.config.analyzed_imports.interfaces {
            let interface_name = interface.name.split('/').last().unwrap_or(&interface.name);
            for method in &interface.methods {
                if method.name.contains("[constructor]") {
                    if let Some(ret) = &method.return_type {
                        if let crate::go::GoType::OwnHandle(name)
                        | crate::go::GoType::BorrowHandle(name)
                        | crate::go::GoType::Resource(name) = &ret.go_type
                        {
                            let prefixed_name = format!("{}-{}", interface_name, name);
                            let pointer_interface_name =
                                GoIdentifier::public(format!("p-{}", prefixed_name));
                            let value_type_param =
                                GoIdentifier::public(format!("t-{}-value", prefixed_name));
                            let pointer_type_param =
                                GoIdentifier::public(format!("p-t-{}", prefixed_name));
                            type_params.push((
                                value_type_param,
                                pointer_type_param,
                                pointer_interface_name,
                            ));
                        }
                    }
                }
            }
        }

        // Build receiver with or without type parameters
        let receiver = if !type_params.is_empty() {
            quote!(*$(self.config.instance)[$(for (value_param, pointer_param, _) in &type_params join (, ) => $value_param, $pointer_param)])
        } else {
            quote!(*$(self.config.instance))
        };

        quote_in! { *tokens =>
            $['\n']
            func (i $receiver) $fn_name(
                $['\r']
                ctx $(&self.config.go_imports.context),
                $(for (name, typ) in &params join ($['\r']) => $name $typ,)
            ) $(f.result()) {
                $(for (arg, param) in arg_assignments join ($['\r']) => $arg := $param)
                $(f.body())
            }
        }
    }

    /// Generate exports for a resource interface
    fn generate_interface_exports(
        &self,
        interface_id: wit_bindgen_core::wit_parser::InterfaceId,
        tokens: &mut Tokens<Go>,
    ) {
        let interface = &self.config.resolve.interfaces[interface_id];
        let interface_name = interface
            .name
            .as_ref()
            .and_then(|n| n.split('/').last())
            .unwrap_or("unknown");

        // Check if this interface is also imported (host-provided resources)
        // If so, we don't generate impl structs, constructors, or methods for exports
        // because the host already manages these resources
        let is_imported = self.is_interface_imported(interface_id);

        // Only generate impl structs for resources in interfaces that are NOT imported
        if !is_imported {
            // Find resources in this interface
            for &type_id in interface.types.values() {
                let type_def = &self.config.resolve.types[type_id];

                if matches!(type_def.kind, TypeDefKind::Resource) {
                    if let Some(resource_name) = &type_def.name {
                        // Generate the resource implementation struct
                        self.generate_resource_impl_struct(interface_name, resource_name, tokens);
                    }
                }
            }
        }

        // Generate methods on the instance for each function
        for func in interface.functions.values() {
            match &func.kind {
                FunctionKind::Constructor(_) => {
                    // Skip constructors for imported interfaces (host provides them)
                    if !is_imported {
                        // Get the resource name
                        let resource_def =
                            &self.config.resolve.types[func.kind.resource().unwrap()];
                        if let Some(resource_name) = &resource_def.name {
                            self.generate_constructor_method(
                                func,
                                interface_name,
                                resource_name,
                                tokens,
                            );
                        }
                    }
                }
                FunctionKind::Method(_) => {
                    // Skip methods for imported interfaces (host provides them)
                    if !is_imported {
                        let resource_def =
                            &self.config.resolve.types[func.kind.resource().unwrap()];
                        if let Some(resource_name) = &resource_def.name {
                            self.generate_resource_method(
                                func,
                                interface_name,
                                resource_name,
                                tokens,
                            );
                        }
                    }
                }
                FunctionKind::Static(_) => {
                    // TODO: Handle static resource methods
                }
                FunctionKind::Freestanding => {
                    // Regular function export (interface-level)
                    self.generate_function(func, interface_name, true, tokens);
                }
                FunctionKind::AsyncFreestanding
                | FunctionKind::AsyncMethod(_)
                | FunctionKind::AsyncStatic(_) => {
                    // TODO: Handle async functions
                }
            }
        }
    }

    /// Generate the implementation struct for a resource
    fn generate_resource_impl_struct(
        &self,
        interface_name: &str,
        resource_name: &str,
        tokens: &mut Tokens<Go>,
    ) {
        let prefixed_name = format!("{}-{}", interface_name, resource_name);
        let impl_name = GoIdentifier::private(format!("{}-impl", prefixed_name));
        let handle_type = GoIdentifier::private(format!("{}-handle", prefixed_name));

        // Exported resources don't need generics - just store the module to call wasm
        quote_in! { *tokens =>
            $['\n']
            type $impl_name struct {
                handle $handle_type
                module $(&self.config.go_imports.wazero_api_module)
            }
            $['\n']
        }
    }

    /// Generate a constructor method on the instance
    fn generate_constructor_method(
        &self,
        func: &Function,
        interface_name: &str,
        resource_name: &str,
        tokens: &mut Tokens<Go>,
    ) {
        let prefixed_name = format!("{}-{}", interface_name, resource_name);
        // Create method name with interface prefix: NewTypesAFoo instead of NewFoo
        let method_name = GoIdentifier::public(format!("new-{}", prefixed_name));
        let impl_name = GoIdentifier::private(format!("{}-impl", prefixed_name));
        let handle_type = GoIdentifier::private(format!("{}-handle", prefixed_name));

        // Build parameter list
        let params = func
            .params
            .iter()
            .map(|(name, typ)| {
                let param_name = GoIdentifier::local(name);
                let param_type = crate::resolve_type(typ, self.config.resolve);
                (param_name, param_type)
            })
            .collect::<Vec<_>>();

        // For constructors, we generate the call manually to properly wrap the result
        // Build the full export name
        let full_interface_name = self
            .config
            .world
            .exports
            .iter()
            .find_map(|(key, _)| {
                if let wit_bindgen_core::wit_parser::WorldKey::Interface(id) = key {
                    let iface = &self.config.resolve.interfaces[*id];
                    if let Some(name) = &iface.name {
                        if name == interface_name {
                            if let Some(package_id) = iface.package {
                                let package = &self.config.resolve.packages[package_id];
                                return Some(format!(
                                    "{}:{}/{}",
                                    package.name.namespace, package.name.name, name
                                ));
                            }
                        }
                    }
                }
                None
            })
            .unwrap_or_else(|| interface_name.to_string());
        let export_name = format!("{}#[constructor]{}", full_interface_name, resource_name);

        // Check if any parameters are strings and build string lowering code
        let has_string_params = params
            .iter()
            .any(|(_, typ)| matches!(typ, crate::go::GoType::String));

        // Build call arguments - strings become ptr, len pairs
        let call_args = params
            .iter()
            .flat_map(|(name, typ)| match typ {
                crate::go::GoType::String => {
                    vec![quote!(uint64(ptr_$name)), quote!(uint64(len_$name))]
                }
                _ => vec![quote!(uint64($name))],
            })
            .collect::<Vec<_>>();

        // Collect resource type parameters for instance receiver
        let mut type_params = Vec::new();
        for interface in &self.config.analyzed_imports.interfaces {
            let iface_name = interface.name.split('/').last().unwrap_or(&interface.name);
            for method in &interface.methods {
                if method.name.contains("[constructor]") {
                    if let Some(ret) = &method.return_type {
                        if let crate::go::GoType::OwnHandle(name)
                        | crate::go::GoType::BorrowHandle(name)
                        | crate::go::GoType::Resource(name) = &ret.go_type
                        {
                            let res_prefixed_name = format!("{}-{}", iface_name, name);
                            let pointer_interface_name =
                                GoIdentifier::public(format!("p-{}", res_prefixed_name));
                            let value_type_param =
                                GoIdentifier::public(format!("t-{}-value", res_prefixed_name));
                            let pointer_type_param =
                                GoIdentifier::public(format!("p-t-{}", res_prefixed_name));
                            type_params.push((
                                value_type_param,
                                pointer_type_param,
                                pointer_interface_name,
                            ));
                        }
                    }
                }
            }
        }

        // Build receiver with or without type parameters
        let receiver = if !type_params.is_empty() {
            quote!(*$(self.config.instance)[$(for (value_param, pointer_param, _) in &type_params join (, ) => $value_param, $pointer_param)])
        } else {
            quote!(*$(self.config.instance))
        };

        quote_in! { *tokens =>
            $['\n']
            func (i $receiver) $method_name(
                $['\r']
                ctx $(&self.config.go_imports.context),
                $(for (name, typ) in &params join (,$['\r']) => $name $typ),
            ) *$(&impl_name) {
                $(if has_string_params {
                    memory := i.module.Memory()
                    realloc := i.module.ExportedFunction("cabi_realloc")
                })
                $(for (name, typ) in &params =>
                    $(match typ {
                        crate::go::GoType::String => {
                            ptr_$name, len_$name, err_$name := writeString(ctx, $name, memory, realloc)
                            if err_$name != nil {
                                panic(err_$name)
                            }
                        }
                        _ => {}
                    })
                )
                raw, err := i.module.ExportedFunction($(quoted(&export_name))).Call(ctx$(for arg in &call_args => , $arg))
                if err != nil {
                    panic(err)
                }
                handle := $(&handle_type)(raw[0])
                return &$(&impl_name){
                    handle: handle,
                    module: i.module,
                }
            }
            $['\n']
        }
    }

    /// Generate a method on the resource implementation struct
    fn generate_resource_method(
        &self,
        func: &Function,
        interface_name: &str,
        resource_name: &str,
        tokens: &mut Tokens<Go>,
    ) {
        let method_name = GoIdentifier::from_resource_function(&func.name);
        let prefixed_name = format!("{}-{}", interface_name, resource_name);
        let impl_name = GoIdentifier::private(format!("{}-impl", prefixed_name));

        // Build parameter list (skip first param which is 'self')
        let params = func
            .params
            .iter()
            .skip(1)
            .map(|(name, typ)| {
                let param_name = GoIdentifier::local(name);
                let param_type = crate::resolve_type(typ, self.config.resolve);
                (param_name, param_type)
            })
            .collect::<Vec<_>>();

        // Build return type
        let result = if let Some(wit_type) = &func.result {
            GoResult::Anon(crate::resolve_type(wit_type, self.config.resolve))
        } else {
            GoResult::Empty
        };

        // For methods, we generate the call manually
        // Build the full export name
        let full_interface_name = self
            .config
            .world
            .exports
            .iter()
            .find_map(|(key, _)| {
                if let wit_bindgen_core::wit_parser::WorldKey::Interface(id) = key {
                    let iface = &self.config.resolve.interfaces[*id];
                    if let Some(name) = &iface.name {
                        if name == interface_name {
                            if let Some(package_id) = iface.package {
                                let package = &self.config.resolve.packages[package_id];
                                return Some(format!(
                                    "{}:{}/{}",
                                    package.name.namespace, package.name.name, name
                                ));
                            }
                        }
                    }
                }
                None
            })
            .unwrap_or_else(|| interface_name.to_string());
        let export_name = format!("{}#{}", full_interface_name, &func.name);

        // Check if any parameters are strings
        let has_string_params = params
            .iter()
            .any(|(_, typ)| matches!(typ, crate::go::GoType::String));

        // Build call arguments - strings become ptr, len pairs
        let call_args = params
            .iter()
            .flat_map(|(name, typ)| match typ {
                crate::go::GoType::String => {
                    vec![quote!(uint64(ptr_$name)), quote!(uint64(len_$name))]
                }
                _ => vec![quote!(uint64($name))],
            })
            .collect::<Vec<_>>();

        // TODO(#58): Support wasm64 architecture size
        // Calculate pointer size for wasm32 (4 bytes for u32 pointers)
        let ptr_size = self
            .config
            .sizes
            .size(&wit_bindgen_core::wit_parser::Type::U32)
            .size_wasm32();

        quote_in! { *tokens =>
            $['\n']
            func (r *$(&impl_name)) $method_name($(for (name, typ) in &params join (, ) => $name $typ)) $(&result) {
                $(if has_string_params {
                    memory := r.module.Memory()
                    realloc := r.module.ExportedFunction("cabi_realloc")
                })
                $(for (name, typ) in &params =>
                    $(match typ {
                        crate::go::GoType::String => {
                            ptr_$name, len_$name, err_$name := writeString(context.Background(), $name, memory, realloc)
                            if err_$name != nil {
                                panic(err_$name)
                            }
                        }
                        _ => {}
                    })
                )
                $(match &result {
                    GoResult::Empty => {
                        _, err := r.module.ExportedFunction($(quoted(&export_name))).Call(context.Background(), uint64(r.handle)$(for arg in &call_args => , $arg))
                        if err != nil {
                            panic(err)
                        }
                    }
                    GoResult::Anon(GoType::Uint32) => {
                        raw, err := r.module.ExportedFunction($(quoted(&export_name))).Call(context.Background(), uint64(r.handle)$(for arg in &call_args => , $arg))
                        if err != nil {
                            panic(err)
                        }
                        result := $(&self.config.go_imports.wazero_api_decode_u32)(uint64(raw[0]))
                        return result
                    }
                    GoResult::Anon(GoType::String) => {
                        // Guest exports return a pointer to where (ptr, len) is stored
                        raw, err := r.module.ExportedFunction($(quoted(&export_name))).Call(context.Background(), uint64(r.handle)$(for arg in &call_args => , $arg))
                        if err != nil {
                            panic(err)
                        }

                        // The returned i32 is a pointer to a (ptr, len) pair in memory
                        result_ptr := uint32(raw[0])
                        memory := r.module.Memory()

                        // Read ptr and len from the returned pointer location
                        ptr, ok1 := memory.ReadUint32Le(result_ptr + 0)
                        if !ok1 {
                            panic("failed to read string ptr")
                        }
                        len, ok2 := memory.ReadUint32Le(result_ptr + $ptr_size)
                        if !ok2 {
                            panic("failed to read string len")
                        }

                        // Read the actual string data
                        buf, ok3 := memory.Read(ptr, len)
                        if !ok3 {
                            panic("failed to read string data")
                        }

                        return string(buf)
                    }
                    GoResult::Anon(_) => {
                        // Other types - stub for now
                        panic("unsupported return type for method")
                    }
                })
            }
            $['\n']
        }
    }

    /// Detect if a function has resource parameters or returns
    /// Returns (resource_name, is_param) if found
    fn detect_resource_in_function(&self, func: &Function) -> Option<(String, bool)> {
        // Check parameters for resources
        for (_name, typ) in &func.params {
            if let wit_bindgen_core::wit_parser::Type::Id(id) = typ {
                let type_def = &self.config.resolve.types[*id];
                match &type_def.kind {
                    wit_bindgen_core::wit_parser::TypeDefKind::Resource => {
                        if let Some(resource_name) = &type_def.name {
                            return Some((resource_name.clone(), true));
                        }
                    }
                    wit_bindgen_core::wit_parser::TypeDefKind::Handle(handle) => {
                        // Extract the resource from inside the handle
                        let resource_id = match handle {
                            wit_bindgen_core::wit_parser::Handle::Own(id)
                            | wit_bindgen_core::wit_parser::Handle::Borrow(id) => id,
                        };
                        let resource_def = &self.config.resolve.types[*resource_id];
                        if let Some(resource_name) = &resource_def.name {
                            return Some((resource_name.clone(), true));
                        }
                    }
                    _ => {}
                }
            }
        }

        // Check result for resources
        if let Some(result_type) = &func.result {
            if let wit_bindgen_core::wit_parser::Type::Id(id) = result_type {
                let type_def = &self.config.resolve.types[*id];
                match &type_def.kind {
                    wit_bindgen_core::wit_parser::TypeDefKind::Resource => {
                        if let Some(resource_name) = &type_def.name {
                            return Some((resource_name.clone(), false));
                        }
                    }
                    wit_bindgen_core::wit_parser::TypeDefKind::Handle(handle) => {
                        // Extract the resource from inside the handle
                        let resource_id = match handle {
                            wit_bindgen_core::wit_parser::Handle::Own(id)
                            | wit_bindgen_core::wit_parser::Handle::Borrow(id) => id,
                        };
                        let resource_def = &self.config.resolve.types[*resource_id];
                        if let Some(resource_name) = &resource_def.name {
                            return Some((resource_name.clone(), false));
                        }
                    }
                    _ => {}
                }
            }
        }

        None
    }

    /// Check if an interface is imported (i.e., host-provided)
    fn is_interface_imported(
        &self,
        interface_id: wit_bindgen_core::wit_parser::InterfaceId,
    ) -> bool {
        use wit_bindgen_core::wit_parser::WorldKey;

        for (key, _) in &self.config.world.imports {
            if let WorldKey::Interface(id) = key {
                if *id == interface_id {
                    return true;
                }
            }
        }
        false
    }

    /// Extract interface name from a function's resource parameters
    fn get_resource_interface_name(&self, func: &wit_bindgen_core::wit_parser::Function) -> String {
        use wit_bindgen_core::wit_parser::{Handle, Type, TypeDefKind, TypeOwner};

        // Look through parameters to find a resource handle
        for (_, param_type) in &func.params {
            if let Type::Id(type_id) = param_type {
                let type_def = self
                    .config
                    .resolve
                    .types
                    .get(*type_id)
                    .expect("type not found");

                // Check if it's a handle to a resource
                if let TypeDefKind::Handle(handle) = &type_def.kind {
                    let resource_id = match handle {
                        Handle::Own(id) | Handle::Borrow(id) => id,
                    };

                    let resource_def = self
                        .config
                        .resolve
                        .types
                        .get(*resource_id)
                        .expect("resource not found");

                    // If this is a type alias (from `use iface.{resource}`), follow the reference
                    let actual_resource_id =
                        if let TypeDefKind::Type(Type::Id(id)) = &resource_def.kind {
                            id
                        } else {
                            resource_id
                        };

                    let actual_resource_def = self
                        .config
                        .resolve
                        .types
                        .get(*actual_resource_id)
                        .expect("actual resource not found");

                    // Get the interface that owns this resource
                    match &actual_resource_def.owner {
                        TypeOwner::Interface(iface_id) => {
                            let interface = self
                                .config
                                .resolve
                                .interfaces
                                .get(*iface_id)
                                .expect("interface not found");
                            if let Some(iface_name) = &interface.name {
                                let iface_short_name =
                                    iface_name.split('/').last().unwrap_or(iface_name);
                                return iface_short_name.to_string();
                            }
                        }
                        TypeOwner::World(_) => {
                            // This case should have been handled by following the type alias above
                        }
                        TypeOwner::None => {}
                    }
                }
            }
        }

        // No resource found, return empty string
        String::new()
    }

    fn resolve_type_with_interface(&self, typ: &GoType, interface_name: &str) -> GoType {
        match typ {
            GoType::OwnHandle(name) | GoType::BorrowHandle(name) => {
                let prefixed_name = if interface_name.is_empty() {
                    format!("{}-handle", name)
                } else {
                    format!("{}-{}-handle", interface_name, name)
                };
                GoType::Resource(prefixed_name)
            }
            GoType::Resource(name) => {
                // Check if it's already a handle type (has -handle suffix)
                if name.ends_with("-handle") {
                    typ.clone()
                } else if name.contains('-') {
                    // Already prefixed but needs handle suffix
                    GoType::Resource(format!("{}-handle", name))
                } else {
                    let prefixed_name = if interface_name.is_empty() {
                        format!("{}-handle", name)
                    } else {
                        format!("{}-{}-handle", interface_name, name)
                    };
                    GoType::Resource(prefixed_name)
                }
            }
            _ => typ.clone(),
        }
    }
}

impl FormatInto<Go> for ExportGenerator<'_> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        for item in self.config.world.exports.values() {
            match item {
                WorldItem::Function(func) => {
                    // For freestanding functions with resource params, look up the interface for type resolution
                    // but don't use it as part of the export name (is_interface_export = false)
                    let interface_name = self.get_resource_interface_name(func);
                    self.generate_function(func, &interface_name, false, tokens);
                }
                WorldItem::Interface { id, .. } => {
                    self.generate_interface_exports(*id, tokens);
                }
                WorldItem::Type(_) => {
                    // Type exports are skipped for now
                    // TODO: Implement type exports
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use genco::prelude::*;
    use indexmap::indexmap;
    use wit_bindgen_core::wit_parser::{
        Function, FunctionKind, Resolve, SizeAlign, Type, World, WorldItem, WorldKey,
    };

    use crate::codegen::ir::AnalyzedImports;
    use crate::go::{GoIdentifier, GoImports};

    use super::{ExportConfig, ExportGenerator};

    #[test]
    fn test_generate_function_simple_u32_param() {
        let func = Function {
            name: "add_number".to_string(),
            kind: FunctionKind::Freestanding,
            params: vec![("value".to_string(), Type::U32)],
            result: Some(Type::U32),
            docs: Default::default(),
            stability: Default::default(),
        };

        let world = World {
            name: "test-world".to_string(),
            imports: indexmap! {},
            exports: indexmap! {
                WorldKey::Name("add-number".to_string()) => WorldItem::Function(func.clone())
            },
            docs: Default::default(),
            stability: Default::default(),
            includes: Default::default(),
            include_names: Default::default(),
            package: None,
        };

        let resolve = Resolve::new();
        let mut sizes = SizeAlign::default();
        sizes.fill(&resolve);
        let instance = GoIdentifier::public("TestInstance");
        let go_imports = GoImports::new();

        let analyzed_imports = AnalyzedImports {
            interfaces: vec![],
            standalone_types: vec![],
            standalone_functions: vec![],
            exported_resources: vec![],
            factory_name: GoIdentifier::public("TestFactory"),
            instance_name: GoIdentifier::public("TestInstance"),
            constructor_name: GoIdentifier::public("NewTestFactory"),
        };

        let config = ExportConfig {
            instance: &instance,
            go_imports: &go_imports,
            world: &world,
            resolve: &resolve,
            sizes: &sizes,
            analyzed_imports: &analyzed_imports,
        };

        let generator = ExportGenerator::new(config);
        let mut tokens = Tokens::new();

        // Call the actual generate_function method (world-level, not interface-level)
        generator.generate_function(&func, "test-interface", false, &mut tokens);

        let generated = tokens.to_string().unwrap();
        println!("Generated: {}", generated);

        // Verify basic function structure
        assert!(generated.contains("func (i *TestInstance) AddNumber("));
        assert!(generated.contains("value uint32"));
        assert!(generated.contains("ctx context.Context"));
        assert!(generated.contains(") uint32 {"));

        // Verify function body
        assert!(generated.contains("arg0 := value"));
        assert!(
            generated
                .contains("i.module.ExportedFunction(\"add_number\").Call(ctx, uint64(result0))")
        );
        assert!(generated.contains("if err1 != nil {"));
        assert!(generated.contains("panic(err1)"));
        assert!(generated.contains("results1 := raw1[0]"));
        assert!(generated.contains("result2 := api.DecodeU32(uint64(results1))"));
        assert!(generated.contains("return result2"));
    }
}
