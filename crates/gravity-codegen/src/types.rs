use anyhow::Result;
use genco::prelude::*;
use gravity_go::GoIdentifier;
use heck::ToUpperCamelCase;
use wit_bindgen_core::wit_parser::{
    Enum, Record, Resolve, Type, TypeDef, TypeDefKind, Variant, WorldItem, WorldKey,
};

use crate::context::GenerationContext;
use crate::resolve_type;

/// Generator for type definitions
pub struct TypeGenerator<'a> {
    context: &'a mut GenerationContext,
    resolve: &'a Resolve,
}

impl<'a> TypeGenerator<'a> {
    pub fn new(context: &'a mut GenerationContext, resolve: &'a Resolve) -> Self {
        Self { context, resolve }
    }

    /// Generate all types found in world exports and imports
    pub fn generate_world_types(
        mut self,
        world_exports: &indexmap::IndexMap<WorldKey, WorldItem>,
        world_imports: &indexmap::IndexMap<WorldKey, WorldItem>,
    ) -> Result<()> {
        eprintln!(
            "DEBUG TypeGenerator::generate_world_types called with {} exports, {} imports",
            world_exports.len(),
            world_imports.len()
        );

        // Collect all type IDs referenced in functions
        let mut referenced_types = std::collections::HashSet::new();

        // Check exports for type references
        for (key, item) in world_exports.iter() {
            eprintln!("DEBUG Processing export for types: {:?}", key);
            match item {
                WorldItem::Function(func) => {
                    // Check parameter types
                    for (_name, typ) in func.params.iter() {
                        self.collect_type_references(typ, &mut referenced_types);
                    }
                    // Check result type
                    if let Some(result_typ) = &func.result {
                        self.collect_type_references(result_typ, &mut referenced_types);
                    }
                }
                WorldItem::Interface { id, .. } => {
                    // Check interface functions for type references
                    let interface = &self.resolve.interfaces[*id];
                    for (_name, func) in interface.functions.iter() {
                        for (_param_name, typ) in func.params.iter() {
                            self.collect_type_references(typ, &mut referenced_types);
                        }
                        if let Some(result_typ) = &func.result {
                            self.collect_type_references(result_typ, &mut referenced_types);
                        }
                    }
                }
                WorldItem::Type(typ_id) => {
                    referenced_types.insert(*typ_id);
                }
            }
        }

        // Check imports for type references
        for (key, item) in world_imports.iter() {
            eprintln!("DEBUG Processing import for types: {:?}", key);
            match item {
                WorldItem::Function(func) => {
                    for (_name, typ) in func.params.iter() {
                        self.collect_type_references(typ, &mut referenced_types);
                    }
                    if let Some(result_typ) = &func.result {
                        self.collect_type_references(result_typ, &mut referenced_types);
                    }
                }
                WorldItem::Interface { id, .. } => {
                    let interface = &self.resolve.interfaces[*id];
                    for (_name, func) in interface.functions.iter() {
                        for (_param_name, typ) in func.params.iter() {
                            self.collect_type_references(typ, &mut referenced_types);
                        }
                        if let Some(result_typ) = &func.result {
                            self.collect_type_references(result_typ, &mut referenced_types);
                        }
                    }
                }
                WorldItem::Type(typ_id) => {
                    referenced_types.insert(*typ_id);
                }
            }
        }

        eprintln!(
            "DEBUG Found {} referenced types to generate",
            referenced_types.len()
        );

        // Generate types in dependency order
        let mut generated = std::collections::HashSet::new();
        for type_id in referenced_types {
            self.generate_type_recursive(type_id, &mut generated)?;
        }

        Ok(())
    }

    /// Recursively collect type IDs referenced by a type
    fn collect_type_references(
        &self,
        typ: &Type,
        referenced_types: &mut std::collections::HashSet<wit_bindgen_core::wit_parser::TypeId>,
    ) {
        match typ {
            Type::Id(id) => {
                referenced_types.insert(*id);
                // Also collect types referenced by this type
                if let Some(typedef) = self.resolve.types.get(*id) {
                    self.collect_typedef_references(typedef, referenced_types);
                }
            }
            _ => {
                // Primitive types don't reference other types
            }
        }
    }

    /// Collect type references from a type definition
    fn collect_typedef_references(
        &self,
        typedef: &TypeDef,
        referenced_types: &mut std::collections::HashSet<wit_bindgen_core::wit_parser::TypeId>,
    ) {
        match &typedef.kind {
            TypeDefKind::Record(record) => {
                for field in record.fields.iter() {
                    self.collect_type_references(&field.ty, referenced_types);
                }
            }
            TypeDefKind::Variant(variant) => {
                for case in variant.cases.iter() {
                    if let Some(typ) = &case.ty {
                        self.collect_type_references(typ, referenced_types);
                    }
                }
            }
            TypeDefKind::Enum(_) => {
                // Enums don't reference other types
            }
            TypeDefKind::List(inner) => {
                self.collect_type_references(inner, referenced_types);
            }
            TypeDefKind::Option(inner) => {
                self.collect_type_references(inner, referenced_types);
            }
            TypeDefKind::Result(result) => {
                if let Some(ok) = &result.ok {
                    self.collect_type_references(ok, referenced_types);
                }
                if let Some(err) = &result.err {
                    self.collect_type_references(err, referenced_types);
                }
            }
            TypeDefKind::Tuple(types) => {
                for typ in types.types.iter() {
                    self.collect_type_references(typ, referenced_types);
                }
            }
            TypeDefKind::Type(typ) => {
                self.collect_type_references(typ, referenced_types);
            }
            _ => {
                // Other kinds don't reference types or are not implemented yet
            }
        }
    }

    /// Generate a type and its dependencies recursively
    fn generate_type_recursive(
        &mut self,
        type_id: wit_bindgen_core::wit_parser::TypeId,
        generated: &mut std::collections::HashSet<wit_bindgen_core::wit_parser::TypeId>,
    ) -> Result<()> {
        if generated.contains(&type_id) {
            return Ok(()); // Already generated
        }

        let typedef = self
            .resolve
            .types
            .get(type_id)
            .ok_or_else(|| anyhow::anyhow!("Type ID {:?} not found in resolve", type_id))?;

        eprintln!(
            "DEBUG Generating type: {:?} ({})",
            type_id,
            typedef.name.as_deref().unwrap_or("anonymous")
        );

        // Generate dependencies first
        self.generate_dependencies(typedef, generated)?;

        // Generate this type
        self.generate_type_definition(typedef)?;

        generated.insert(type_id);
        Ok(())
    }

    /// Generate dependencies of a type
    fn generate_dependencies(
        &mut self,
        typedef: &TypeDef,
        generated: &mut std::collections::HashSet<wit_bindgen_core::wit_parser::TypeId>,
    ) -> Result<()> {
        let mut dependencies = std::collections::HashSet::new();
        self.collect_typedef_references(typedef, &mut dependencies);

        for dep_id in dependencies {
            if !generated.contains(&dep_id) {
                self.generate_type_recursive(dep_id, generated)?;
            }
        }

        Ok(())
    }

    /// Generate a single type definition
    fn generate_type_definition(&mut self, typedef: &TypeDef) -> Result<()> {
        match &typedef.kind {
            TypeDefKind::Record(record) => {
                let name = typedef
                    .name
                    .as_deref()
                    .unwrap_or("Anonymous")
                    .to_upper_camel_case();
                self.generate_record_type(record, &name)?;
            }
            TypeDefKind::Variant(variant) => {
                let name = typedef
                    .name
                    .as_deref()
                    .unwrap_or("Anonymous")
                    .to_upper_camel_case();
                self.generate_variant_type(variant, &name)?;
            }
            TypeDefKind::Enum(enum_) => {
                let name = typedef
                    .name
                    .as_deref()
                    .unwrap_or("Anonymous")
                    .to_upper_camel_case();
                self.generate_enum_type(enum_, &name)?;
            }
            TypeDefKind::List(_)
            | TypeDefKind::Option(_)
            | TypeDefKind::Result(_)
            | TypeDefKind::Tuple(_)
            | TypeDefKind::Type(_) => {
                // These are handled inline by resolve_type, no separate definition needed
                eprintln!(
                    "DEBUG Skipping type definition for inline type: {:?}",
                    typedef.kind
                );
            }
            _ => {
                eprintln!("DEBUG Unhandled type definition kind: {:?}", typedef.kind);
            }
        }

        Ok(())
    }

    /// Generate a record (struct) type
    fn generate_record_type(&mut self, record: &Record, name: &str) -> Result<()> {
        eprintln!("DEBUG Generating record type: {}", name);

        let type_name = GoIdentifier::Public { name };

        // Pre-process field types to handle ValueOrOk -> Pointer conversion for optional fields
        let field_data: Vec<_> = record
            .fields
            .iter()
            .map(|field| {
                let field_type = match resolve_type(&field.ty, self.resolve)? {
                    gravity_go::GoType::ValueOrOk(inner_type) => {
                        gravity_go::GoType::Pointer(inner_type)
                    }
                    other => other,
                };
                let field_name = field.name.to_upper_camel_case();
                Ok((field_name, field_type))
            })
            .collect::<Result<Vec<_>>>()?;

        quote_in! { self.context.out =>
            $['\n']
            type $type_name struct {
                $(for (field_name, field_type) in field_data.iter() join ($['\r']) =>
                    $(GoIdentifier::Public { name: field_name }) $field_type)
            }
            $['\n']
        }

        eprintln!("DEBUG Successfully generated record type: {}", name);
        Ok(())
    }

    /// Generate a variant (enum-like) type
    fn generate_variant_type(&mut self, variant: &Variant, name: &str) -> Result<()> {
        eprintln!("DEBUG Generating variant type: {}", name);

        // Generate the base interface
        let interface_name = GoIdentifier::Public { name };
        let discriminant_method = format!("is{}", name);

        quote_in! { self.context.out =>
            $['\n']
            type $interface_name interface {
                $(&discriminant_method)()
            }
            $['\n']
        }

        // Generate case types
        for case in variant.cases.iter() {
            let case_name = format!("{}{}", name, case.name.to_upper_camel_case());
            let case_type = GoIdentifier::Public { name: &case_name };
            let discriminant_method_ref = &discriminant_method;

            match &case.ty {
                Some(typ) => {
                    let go_type = resolve_type(typ, self.resolve)?;
                    quote_in! { self.context.out =>
                        type $(&case_type) $go_type
                        func ($(&case_type)) $(discriminant_method_ref)() {}
                        $['\n']
                    }
                }
                None => {
                    // Unit variant - empty struct
                    quote_in! { self.context.out =>
                        type $(&case_type) struct {}
                        func ($(&case_type)) $(discriminant_method_ref)() {}
                        $['\n']
                    }
                }
            }
        }

        eprintln!("DEBUG Successfully generated variant type: {}", name);
        Ok(())
    }

    /// Generate an enum type
    fn generate_enum_type(&mut self, enum_: &Enum, name: &str) -> Result<()> {
        eprintln!("DEBUG Generating enum type: {}", name);

        let type_name = GoIdentifier::Public { name };

        quote_in! { self.context.out =>
            $['\n']
            type $(&type_name) uint32
            $['\n']
            const (
                $(for (i, case) in enum_.cases.iter().enumerate() join ($['\r']) =>
                    $(GoIdentifier::Public {
                        name: &format!("{}{}", name, case.name.to_upper_camel_case())
                    }) $(&type_name) = $i)
            )
            $['\n']
        }

        eprintln!("DEBUG Successfully generated enum type: {}", name);
        Ok(())
    }
}
