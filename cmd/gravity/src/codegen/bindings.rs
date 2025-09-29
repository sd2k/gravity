use genco::{prelude::*, tokens::Tokens};
use wit_bindgen_core::wit_parser::{Record, Resolve, Type, TypeDef, TypeDefKind};

use crate::{
    codegen::wasm::{Wasm, WasmData},
    go::*,
    resolve_type,
};

/// The WIT bindings for a world.
pub struct Bindings {
    /// The cumulative output tokens containing the Go bindings.
    // TODO(#16): Don't use the internal bindings.out field
    pub out: Tokens<Go>,

    /// The identifier of the Go variable containing the WebAssembly bytes.
    raw_wasm_var: GoIdentifier,
}

impl Bindings {
    /// Creates a new bindings generator for the selected world.
    pub fn new(world: &str) -> Self {
        let wasm_var = GoIdentifier::private(format!("wasm-file-{world}"));
        Self {
            // world,
            out: Tokens::new(),
            raw_wasm_var: wasm_var,
        }
    }

    /// Adds the given Wasm to the bindings.
    pub fn include_wasm(&mut self, wasm: WasmData) {
        Wasm::new(&self.raw_wasm_var, wasm).format_into(&mut self.out)
    }

    pub fn define_type(&mut self, typ_def: &TypeDef, resolve: &Resolve) {
        let TypeDef { name, kind, .. } = typ_def;
        match kind {
            TypeDefKind::Record(Record { fields }) => {
                let name = GoIdentifier::public(name.as_deref().expect("record to have a name"));
                let fields = fields.iter().map(|field| {
                    (
                        GoIdentifier::public(&field.name),
                        resolve_type(&field.ty, resolve),
                    )
                });

                quote_in! { self.out =>
                    $['\n']
                    type $name struct {
                        $(for (name, typ) in fields join ($['\r']) => $name $typ)
                    }
                }
            }
            TypeDefKind::Resource => todo!("TODO(#5): implement resources"),
            TypeDefKind::Handle(_) => todo!("TODO(#5): implement resources"),
            TypeDefKind::Flags(_) => todo!("TODO(#4):generate flags type definition"),
            TypeDefKind::Tuple(_) => todo!("TODO(#4):generate tuple type definition"),
            TypeDefKind::Variant(_) => {
                // TODO(#4): Generate aliases if the variant name doesn't match the struct name
            }
            TypeDefKind::Enum(inner) => {
                let name = name.clone().expect("enum to have a name");
                let enum_type = &GoIdentifier::private(&name);

                let enum_interface = GoIdentifier::public(&name);

                let enum_function = &GoIdentifier::private(format!("is-{}", &name));

                let variants = inner
                    .cases
                    .iter()
                    .map(|variant| GoIdentifier::public(&variant.name));

                quote_in! { self.out =>
                    $['\n']
                    type $enum_interface interface {
                        $enum_function()
                    }

                    type $enum_type int

                    func ($enum_type) $enum_function() {}

                    const (
                        $(for name in variants join ($['\r']) => $name $enum_type = iota)
                    )
                }
            }
            TypeDefKind::Option(_) => todo!("TODO(#4): generate option type definition"),
            TypeDefKind::Result(_) => todo!("TODO(#4): generate result type definition"),
            TypeDefKind::List(_) => todo!("TODO(#4): generate list type definition"),
            TypeDefKind::Future(_) => todo!("TODO(#4): generate future type definition"),
            TypeDefKind::Stream(_) => todo!("TODO(#4): generate stream type definition"),
            TypeDefKind::Type(Type::Id(_)) => {
                // TODO(#4):  Only skip this if we have already generated the type
            }
            TypeDefKind::Type(Type::Bool) => todo!("TODO(#4): generate bool type alias"),
            TypeDefKind::Type(Type::U8) => todo!("TODO(#4): generate u8 type alias"),
            TypeDefKind::Type(Type::U16) => todo!("TODO(#4): generate u16 type alias"),
            TypeDefKind::Type(Type::U32) => todo!("TODO(#4): generate u32 type alias"),
            TypeDefKind::Type(Type::U64) => todo!("TODO(#4): generate u64 type alias"),
            TypeDefKind::Type(Type::S8) => todo!("TODO(#4): generate s8 type alias"),
            TypeDefKind::Type(Type::S16) => todo!("TODO(#4): generate s16 type alias"),
            TypeDefKind::Type(Type::S32) => todo!("TODO(#4): generate s32 type alias"),
            TypeDefKind::Type(Type::S64) => todo!("TODO(#4): generate s64 type alias"),
            TypeDefKind::Type(Type::F32) => todo!("TODO(#4): generate f32 type alias"),
            TypeDefKind::Type(Type::F64) => todo!("TODO(#4): generate f64 type alias"),
            TypeDefKind::Type(Type::Char) => todo!("TODO(#4): generate char type alias"),
            TypeDefKind::Type(Type::String) => {
                let name =
                    GoIdentifier::public(name.as_deref().expect("string alias to have a name"));
                // TODO(#4): We might want a Type Definition (newtype) instead of Type Alias here
                quote_in! { self.out =>
                    $['\n']
                    type $name = string
                }
            }
            TypeDefKind::Type(Type::ErrorContext) => {
                todo!("TODO(#4): generate error context definition")
            }
            TypeDefKind::FixedSizeList(_, _) => {
                todo!("TODO(#4): generate fixed size list definition")
            }
            TypeDefKind::Unknown => panic!("cannot generate Unknown type"),
        }
    }
}
