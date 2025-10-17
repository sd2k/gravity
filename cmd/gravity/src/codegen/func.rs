use std::mem;

use genco::{prelude::*, tokens::static_literal};
use wit_bindgen_core::{
    abi::{Bindgen, Instruction},
    wit_parser::{
        Alignment, ArchitectureSize, Handle, Resolve, Result_, SizeAlign, Type, TypeDefKind,
    },
};

use crate::{
    go::{
        GoIdentifier, GoResult, GoType, Operand, comment,
        imports::{
            ERRORS_NEW, REFLECT_VALUE_OF, WAZERO_API_DECODE_F32, WAZERO_API_DECODE_F64,
            WAZERO_API_DECODE_I32, WAZERO_API_DECODE_U32, WAZERO_API_ENCODE_F32,
            WAZERO_API_ENCODE_F64, WAZERO_API_ENCODE_I32, WAZERO_API_ENCODE_U32,
        },
    },
    resolve_type, resolve_wasm_type,
};

/// The direction of a function.
///
/// Functions in the Component Model can be imported into a world or
/// exported from a world.

/// Context for resource handling in imported functions
#[derive(Clone)]
struct ResourceContext {
    /// Interface name (e.g., "types-a")
    interface_name: String,
    /// Resource name (e.g., "foo")
    resource_name: String,
    /// Resource table variable name (e.g., "typesAFooResourceTable")
    table_var: String,
}

/// Expression for getting the pointer size in an imported function. This will use the architecture
/// size from the factory.
const IMPORT_PTRSIZE_EXPR: &str = "factory.architecture.PointerSize()";
/// Expression for getting the pointer size in an exported function. This will use the architecture
/// field of the 'instance' struct.
const EXPORT_PTRSIZE_EXPR: &str = "i.architecture.PointerSize()";

enum Direction<'a> {
    /// The function is imported into the world.
    Import {
        /// The name of the parameter representing the interface instance
        /// in the generated host binding function.
        param_name: &'a GoIdentifier,
        /// Optional resource context for resource constructors and methods
        resource_context: Option<ResourceContext>,
    },
    /// The function is exported from the world.
    #[allow(dead_code, reason = "halfway through refactor of func bindings")]
    Export {
        /// Optional resource context for resource parameters/returns
        resource_context: Option<ResourceContext>,
    },
}

pub struct Func<'a> {
    direction: Direction<'a>,
    args: Vec<String>,
    result: GoResult,
    tmp: usize,
    body: Tokens<Go>,
    block_storage: Vec<Tokens<Go>>,
    blocks: Vec<(Tokens<Go>, Vec<Operand>)>,
    sizes: &'a SizeAlign,
    /// Override the export name used in CallWasm instructions (for interface-qualified names)
    export_name: Option<String>,
}

impl<'a> Func<'a> {
    /// Create a new exported function.
    #[allow(dead_code, reason = "halfway through refactor of func bindings")]
    pub fn export(result: GoResult, sizes: &'a SizeAlign) -> Self {
        Self {
            direction: Direction::Export {
                resource_context: None,
            },
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
            export_name: None,
        }
    }

    /// Create a new exported function with resource context.
    pub fn export_with_resource(
        result: GoResult,
        sizes: &'a SizeAlign,
        interface_name: String,
        resource_name: String,
    ) -> Self {
        let interface_pascal = interface_name
            .split('-')
            .map(|s| {
                let mut c = s.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join("");
        let resource_pascal = resource_name
            .split('-')
            .map(|s| {
                let mut c = s.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join("");

        let interface_camel = {
            let mut c = interface_pascal.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
            }
        };

        let table_var = format!("{}{}ResourceTable", interface_camel, resource_pascal);

        Self {
            direction: Direction::Export {
                resource_context: Some(ResourceContext {
                    interface_name,
                    resource_name,
                    table_var,
                }),
            },
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
            export_name: None,
        }
    }

    /// Create a new exported function.
    pub fn import(param_name: &'a GoIdentifier, result: GoResult, sizes: &'a SizeAlign) -> Self {
        Self {
            direction: Direction::Import {
                param_name,
                resource_context: None,
            },
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
            export_name: None,
        }
    }

    /// Create a new imported function with resource context.
    pub fn import_with_resource(
        param_name: &'a GoIdentifier,
        result: GoResult,
        sizes: &'a SizeAlign,
        interface_name: String,
        resource_name: String,
        table_var: String,
    ) -> Self {
        Self {
            direction: Direction::Import {
                param_name,
                resource_context: Some(ResourceContext {
                    interface_name,
                    resource_name,
                    table_var,
                }),
            },
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
            export_name: None,
        }
    }

    fn tmp(&mut self) -> usize {
        let ret = self.tmp;
        self.tmp += 1;
        ret
    }

    pub fn args(&self) -> &[String] {
        &self.args
    }

    pub fn result(&self) -> &GoResult {
        &self.result
    }

    pub fn body(&self) -> &Tokens<Go> {
        &self.body
    }

    /// Set the export name to use in CallWasm instructions (for interface-qualified names)
    pub fn set_export_name(&mut self, name: String) {
        self.export_name = Some(name);
    }

    fn push_arg(&mut self, value: &str) {
        self.args.push(value.into())
    }

    fn pop_block(&mut self) -> (Tokens<Go>, Vec<Operand>) {
        self.blocks.pop().expect("should have block to pop")
    }
}

impl Bindgen for Func<'_> {
    type Operand = Operand;

    fn emit(
        &mut self,
        resolve: &Resolve,
        inst: &Instruction<'_>,
        operands: &mut Vec<Self::Operand>,
        results: &mut Vec<Self::Operand>,
    ) {
        let iter_element = "e";
        let iter_base = "base";

        let payload = &format!("{:?}", inst);
        quote_in! {
            self.body =>
            $(comment([payload]))
        }
        match inst {
            Instruction::GetArg { nth } => {
                let arg = &format!("arg{nth}");
                self.push_arg(arg);
                results.push(Operand::SingleValue(arg.into()));
            }
            Instruction::ConstZero { tys } => {
                for _ in tys.iter() {
                    results.push(Operand::Literal("0".into()))
                }
            }
            Instruction::StringLower { realloc: None } => todo!("implement instruction: {inst:?}"),
            Instruction::StringLower {
                realloc: Some(realloc_name),
            } => {
                let tmp = self.tmp();
                let ptr = &format!("ptr{tmp}");
                let len = &format!("len{tmp}");
                let err = &format!("err{tmp}");
                let default = &format!("default{tmp}");
                let memory = &format!("memory{tmp}");
                let realloc = &format!("realloc{tmp}");
                let operand = &operands[0];
                match self.direction {
                    Direction::Export { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $memory := i.module.Memory()
                            $realloc := i.module.ExportedFunction($(quoted(*realloc_name)))
                            $ptr, $len, $err := writeString(ctx, $operand, $memory, $realloc)
                            $(match &self.result {
                                GoResult::Anon(GoType::ValueOrError(typ)) => {
                                    if $err != nil {
                                        var $default $(typ.as_ref())
                                        return $default, $err
                                    }
                                }
                                GoResult::Anon(GoType::Error) => {
                                    if $err != nil {
                                        return $err
                                    }
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    if $err != nil {
                                        panic($err)
                                    }
                                }
                            })
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $memory := mod.Memory()
                            $realloc := mod.ExportedFunction($(quoted(*realloc_name)))
                            $ptr, $len, $err := writeString(ctx, $operand, $memory, $realloc)
                            if $err != nil {
                                panic($err)
                            }
                        };
                    }
                }
                results.push(Operand::SingleValue(ptr.into()));
                results.push(Operand::SingleValue(len.into()));
            }
            Instruction::CallWasm { name, .. } => {
                let tmp = self.tmp();
                let raw = &format!("raw{tmp}");
                let ret = &format!("results{tmp}");
                let err = &format!("err{tmp}");
                let default = &format!("default{tmp}");
                // Use export_name if set, otherwise use the instruction's name
                let export_name = self.export_name.as_deref().unwrap_or(name);
                // TODO(#17): Wrapping every argument in `uint64` is bad and we should instead be looking
                // at the types and converting with proper guards in place
                quote_in! { self.body =>
                    $['\r']
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            $raw, $err := i.module.ExportedFunction($(quoted(export_name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            if $err != nil {
                                var $default $(typ.as_ref())
                                return $default, $err
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            $raw, $err := i.module.ExportedFunction($(quoted(export_name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            if $err != nil {
                                return $err
                            }
                        }
                        GoResult::Anon(_) => {
                            $raw, $err := i.module.ExportedFunction($(quoted(export_name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if $err != nil {
                                panic($err)
                            }
                        }
                        GoResult::Empty => {
                            _, $err := i.module.ExportedFunction($(quoted(export_name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if $err != nil {
                                panic($err)
                            }
                        }
                    })

                    $(if self.result.needs_cleanup() {
                        $(comment(&[
                            "The cleanup via `cabi_post_*` cleans up the memory in the guest. By",
                            "deferring this, we ensure that no memory is corrupted before the function",
                            "is done accessing it."
                        ]))
                        defer func() {
                            if postFn := i.module.ExportedFunction($(quoted(format!("cabi_post_{name}")))); postFn != nil {
                                if _, err := postFn.Call(ctx, $raw...); err != nil {
                                    $(comment(&[
                                        "If we get an error during cleanup, something really bad is",
                                        "going on, so we panic. Also, you can't return the error from",
                                        "the `defer`"
                                    ]))
                                    panic($ERRORS_NEW("failed to cleanup"))
                                }
                            }
                        }()
                    })

                    $(match &self.result {
                        GoResult::Anon(_) => $ret := $raw[0],
                        GoResult::Empty => (),
                    })
                };
                match self.result {
                    GoResult::Empty => (),
                    GoResult::Anon(_) => results.push(Operand::SingleValue(ret.into())),
                }
            }
            Instruction::I32Load8U { offset } => {
                let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadByte(uint32($operand) + uint32($offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read byte from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read byte from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read byte from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromBool => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    var $(&value) uint32
                    if $operand {
                        $(&value) = 1
                    } else {
                        $(&value) = 0
                    }
                }
                results.push(Operand::SingleValue(value))
            }
            Instruction::BoolFromI32 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := $operand != 0
                }
                results.push(Operand::SingleValue(value))
            }
            Instruction::I32FromU32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                match self.direction {
                    Direction::Import { .. } => {
                        // For host functions (imports), just pass through the value
                        // The value is already uint32 and doesn't need encoding
                        quote_in! { self.body =>
                            $['\r']
                            $result := $operand
                        };
                    }
                    Direction::Export { .. } => {
                        // For exports, encode the value for passing to Wasm
                        quote_in! { self.body =>
                            $['\r']
                            $result := $WAZERO_API_ENCODE_U32($operand)
                        };
                    }
                }
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::U32FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_DECODE_U32(uint64($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::PointerLoad { offset } => {
                let offset = &offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                let tmp = self.tmp();
                let ptr = &format!("ptr{tmp}");
                let tmp_ptr = &format!("tmpPtr{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    var $ptr uint64
                    var $ok bool
                    switch i.architecture {
                    case ArchitectureWasm64:
                        $ptr, $ok = i.module.Memory().ReadUint64Le(uint32($operand) + uint32($offset))
                    case ArchitectureWasm32:
                        var $tmp_ptr uint32
                        $tmp_ptr, $ok = i.module.Memory().ReadUint32Le(uint32($operand) + uint32($offset))
                        $ptr = uint64($tmp_ptr)
                    }
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read pointer from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read pointer from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read pointer from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(ptr.into()));
            }
            Instruction::LengthLoad { offset } => {
                let offset = &offset.format_term(EXPORT_PTRSIZE_EXPR, true);
                let tmp = self.tmp();
                let len = &format!("len{tmp}");
                let tmp_len = &format!("tmpLen{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    var $len uint64
                    var $ok bool
                    switch i.architecture {
                    case ArchitectureWasm64:
                        $len, $ok = i.module.Memory().ReadUint64Le(uint32($operand) + uint32($offset))
                    case ArchitectureWasm32:
                        var $tmp_len uint32
                        $tmp_len, $ok = i.module.Memory().ReadUint32Le(uint32($operand) + uint32($offset))
                        $len = uint64($tmp_len)
                    }
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read length from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read length from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read length from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(len.into()));
            }
            Instruction::I32Load { offset } => {
                let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint32Le(uint32($operand) + uint32($offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read i32 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read i32 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read i32 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::StringLift => {
                let tmp = self.tmp();
                let buf = &format!("buf{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let str = &format!("str{tmp}");
                let ptr = &operands[0];
                let len = &operands[1];
                match self.direction {
                    Direction::Export { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $buf, $ok := i.module.Memory().Read(uint32($ptr), uint32($len))
                            $(match &self.result {
                                GoResult::Anon(GoType::ValueOrError(typ)) => {
                                    if !$ok {
                                        var $default $(typ.as_ref())
                                        return $default, $ERRORS_NEW("failed to read bytes from memory")
                                    }
                                }
                                GoResult::Anon(GoType::Error) => {
                                    if !$ok {
                                        return $ERRORS_NEW("failed to read bytes from memory")
                                    }
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    if !$ok {
                                        panic($ERRORS_NEW("failed to read bytes from memory"))
                                    }
                                }
                            })
                            $str := string($buf)
                        };
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $buf, $ok := mod.Memory().Read(uint32($ptr), uint32($len))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read bytes from memory"))
                            }
                            $str := string($buf)
                        };
                    }
                }
                results.push(Operand::SingleValue(str.into()));
            }
            Instruction::ResultLift {
                result:
                    Result_ {
                        ok: Some(typ),
                        err: Some(Type::String),
                    },
                ..
            } => {
                let (err_block, err_results) = self.pop_block();
                assert_eq!(err_results.len(), 1);
                let err_op = &err_results[0];

                let (ok_block, ok_results) = self.pop_block();
                assert_eq!(ok_results.len(), 1);
                let ok_op = &ok_results[0];

                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let err = &format!("err{tmp}");
                let typ = resolve_type(typ, resolve);
                let tag = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    var $value $typ
                    var $err error
                    switch $tag {
                    case 0:
                        $ok_block
                        $value = $ok_op
                    case 1:
                        $err_block
                        $err = $ERRORS_NEW($err_op)
                    default:
                        $err = $ERRORS_NEW("invalid variant discriminant for expected")
                    }
                };

                results.push(Operand::DoubleValue(value.into(), err.into()));
            }
            Instruction::ResultLift {
                result:
                    Result_ {
                        ok: None,
                        err: Some(Type::String),
                    },
                ..
            } => {
                let (err_block, err_results) = self.pop_block();
                assert_eq!(err_results.len(), 1);
                let err_op = &err_results[0];

                let (ok_block, ok_results) = self.pop_block();
                assert_eq!(ok_results.len(), 0);

                let tmp = self.tmp();
                let err = &format!("err{tmp}");
                let tag = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    var $err error
                    switch $tag {
                    case 0:
                        $ok_block
                    case 1:
                        $err_block
                        $err = $ERRORS_NEW($err_op)
                    default:
                        $err = $ERRORS_NEW("invalid variant discriminant for expected")
                    }
                };

                results.push(Operand::SingleValue(err.into()));
            }
            Instruction::ResultLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::Return { amt, .. } => {
                if *amt != 0 {
                    let operand = &operands[0];
                    quote_in! { self.body =>
                        $['\r']
                        return $operand
                    };
                }
            }
            Instruction::CallInterface { func, .. } => {
                let ident = GoIdentifier::from_resource_function(&func.name);
                let tmp = self.tmp();
                let args = quote!($(for op in operands.iter() join (, ) => $op));
                let returns = match &func.result {
                    None => GoType::Nothing,
                    Some(typ) => resolve_type(typ, resolve),
                };
                let value = &format!("value{tmp}");
                let err = &format!("err{tmp}");
                let ok = &format!("ok{tmp}");

                // Check if this is a resource constructor or method
                let is_constructor = func.name.starts_with("[constructor]");
                let is_method = func.name.starts_with("[method]");

                // Check if first parameter is a resource type
                let first_param_is_resource = func.params.first().map_or(false, |(_, typ)| {
                    if let wit_bindgen_core::wit_parser::Type::Id(id) = typ {
                        let type_def = &resolve.types[*id];
                        matches!(
                            type_def.kind,
                            wit_bindgen_core::wit_parser::TypeDefKind::Handle(_)
                                | wit_bindgen_core::wit_parser::TypeDefKind::Resource
                        )
                    } else {
                        false
                    }
                });

                match &self.direction {
                    Direction::Export { .. } => todo!("TODO(#10): handle export direction"),
                    Direction::Import {
                        param_name,
                        resource_context,
                    } => {
                        if is_constructor && resource_context.is_some() {
                            // Constructor: call interface method, store in table, return handle
                            quote_in! { self.body =>
                                $['\r']
                                $(match returns {
                                    GoType::OwnHandle(_) | GoType::Resource(_) => {
                                        $value := $(*param_name).$ident(ctx, $args)
                                    }
                                    _ => $(comment(&["Unexpected return type for constructor"]))
                                })
                            }
                        } else if is_method && resource_context.is_some() {
                            // Method: lookup resource from table, call method on resource
                            let ctx = resource_context.as_ref().unwrap();
                            let table_var = &ctx.table_var;
                            let resource_var = &format!("resource{tmp}");
                            let ok_var = &format!("ok{tmp}");
                            // First operand should be the handle (self parameter)
                            let handle_operand = &operands[0];
                            // Remaining operands are method parameters
                            let method_args = if operands.len() > 1 {
                                quote!($(for op in operands.iter().skip(1) join (, ) => $op))
                            } else {
                                quote!()
                            };

                            quote_in! { self.body =>
                                $['\r']
                                $resource_var, $ok_var := $table_var.get($handle_operand)
                                if !$ok_var {
                                    panic("invalid resource handle")
                                }
                                $(match returns {
                                    GoType::Nothing => $resource_var.$(&ident)(ctx$(if !method_args.is_empty() => , $method_args)),
                                    GoType::Bool | GoType::Uint32 | GoType::Interface | GoType::String | GoType::UserDefined(_) => $value := $resource_var.$(&ident)(ctx$(if !method_args.is_empty() => , $method_args)),
                                    GoType::Error => $err := $resource_var.$(&ident)(ctx$(if !method_args.is_empty() => , $method_args)),
                                    GoType::ValueOrError(_) => {
                                        $value, $err := $resource_var.$(&ident)(ctx$(if !method_args.is_empty() => , $method_args))
                                    }
                                    GoType::ValueOrOk(_) => {
                                        $value, $ok := $resource_var.$(&ident)(ctx$(if !method_args.is_empty() => , $method_args))
                                    }
                                    _ => $(comment(&["TODO(#9): handle return type"]))
                                })
                            }
                        } else if resource_context.is_some()
                            && !operands.is_empty()
                            && first_param_is_resource
                        {
                            // Freestanding function with resource parameter: lookup resource from table
                            let ctx = resource_context.as_ref().unwrap();
                            let table_var = &ctx.table_var;
                            let resource_var = &format!("resource{tmp}");
                            let ok_var = &format!("ok{tmp}");
                            // First operand should be the handle (resource parameter)
                            let handle_operand = &operands[0];
                            // Remaining operands are other parameters
                            let remaining_args = if operands.len() > 1 {
                                quote!($(for op in operands.iter().skip(1) join (, ) => , $op))
                            } else {
                                quote!()
                            };

                            quote_in! { self.body =>
                                $['\r']
                                $resource_var, $ok_var := $table_var.get($handle_operand)
                                if !$ok_var {
                                    panic("invalid resource handle")
                                }
                                $(match returns {
                                    GoType::Nothing => $(*param_name).$ident(ctx, $resource_var$remaining_args),
                                    GoType::Bool | GoType::Uint32 | GoType::Interface | GoType::String | GoType::UserDefined(_) | GoType::OwnHandle(_) | GoType::BorrowHandle(_) | GoType::Resource(_) => $value := $(*param_name).$ident(ctx, $resource_var$remaining_args),
                                    GoType::Error => $err := $(*param_name).$ident(ctx, $resource_var$remaining_args),
                                    GoType::ValueOrError(_) => {
                                        $value, $err := $(*param_name).$ident(ctx, $resource_var$remaining_args)
                                    }
                                    GoType::ValueOrOk(_) => {
                                        $value, $ok := $(*param_name).$ident(ctx, $resource_var$remaining_args)
                                    }
                                    _ => $(comment(&["TODO(#9): handle return type"]))
                                })
                            }
                        } else {
                            // Regular interface call (not a resource constructor or method)
                            quote_in! { self.body =>
                                $['\r']
                                $(match returns {
                                    GoType::Nothing => $(*param_name).$ident(ctx, $args),
                                    GoType::Bool | GoType::Uint32 | GoType::Interface | GoType::String | GoType::UserDefined(_) | GoType::OwnHandle(_) | GoType::BorrowHandle(_) | GoType::Resource(_) => $value := $(*param_name).$ident(ctx, $args),
                                    GoType::Error => $err := $(*param_name).$ident(ctx, $args),
                                    GoType::ValueOrError(_) => {
                                        $value, $err := $(*param_name).$ident(ctx, $args)
                                    }
                                    GoType::ValueOrOk(_) => {
                                        $value, $ok := $(*param_name).$ident(ctx, $args)
                                    }
                                    _ => $(comment(&["TODO(#9): handle return type"]))
                                })
                            }
                        }
                    }
                }
                match returns {
                    GoType::Nothing => (),
                    GoType::Bool
                    | GoType::Uint32
                    | GoType::Interface
                    | GoType::UserDefined(_)
                    | GoType::String
                    | GoType::OwnHandle(_)
                    | GoType::BorrowHandle(_)
                    | GoType::Resource(_) => {
                        results.push(Operand::SingleValue(value.into()));
                    }
                    GoType::Error => {
                        results.push(Operand::SingleValue(err.into()));
                    }
                    GoType::ValueOrError(_) => {
                        results.push(Operand::DoubleValue(value.into(), err.into()));
                    }
                    GoType::ValueOrOk(_) => {
                        results.push(Operand::DoubleValue(value.into(), ok.into()))
                    }
                    _ => todo!("TODO(#9): handle return type - {returns:?}"),
                }
            }
            Instruction::VariantPayloadName => {
                results.push(Operand::SingleValue("variantPayload".into()));
            }
            Instruction::I32Const { val } => results.push(Operand::Literal(val.to_string())),
            Instruction::I32Store8 { offset } => {
                let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                let tag = &operands[0];
                let ptr = &operands[1];
                if let Operand::Literal(byte) = tag {
                    match &self.direction {
                        Direction::Export { .. } => {
                            quote_in! { self.body =>
                                $['\r']
                                i.module.Memory().WriteByte(uint32($ptr)+uint32($offset), $byte)
                            }
                        }
                        Direction::Import { .. } => {
                            quote_in! { self.body =>
                                $['\r']
                                mod.Memory().WriteByte(uint32($ptr)+uint32($offset), $byte)
                            }
                        }
                    }
                } else {
                    let tmp = self.tmp();
                    let byte = format!("byte{tmp}");
                    match &self.direction {
                        Direction::Export { .. } => {
                            quote_in! { self.body =>
                                $['\r']
                                var $(&byte) uint8
                                switch $tag {
                                case 0:
                                    $(&byte) = 0
                                case 1:
                                    $(&byte) = 1
                                default:
                                    $(comment(["TODO(#8): Return an error if the return type allows it"]))
                                    panic($ERRORS_NEW("invalid int8 value encountered"))
                                }
                                i.module.Memory().WriteByte(uint32($ptr+$offset), $byte)
                            }
                        }
                        Direction::Import { .. } => {
                            quote_in! { self.body =>
                                $['\r']
                                var $(&byte) uint8
                                switch $tag {
                                case 0:
                                    $(&byte) = 0
                                case 1:
                                    $(&byte) = 1
                                default:
                                    panic($ERRORS_NEW("invalid int8 value encountered"))
                                }
                                mod.Memory().WriteByte(uint32($ptr+$offset), $byte)
                            }
                        }
                    }
                }
            }
            Instruction::I32Store { offset } => {
                let tag = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export { .. } => {
                        let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint32Le(uint32($ptr)+uint32($offset), uint32($tag))
                        }
                    }
                    Direction::Import { .. } => {
                        let offset = offset.format_term(IMPORT_PTRSIZE_EXPR, false);
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint32Le(uint32($ptr) + uint32($offset), uint32($tag))
                        }
                    }
                }
            }
            Instruction::LengthStore { offset } => {
                let len = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export { .. } => {
                        let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint32Le(uint32($ptr) + uint32($offset), uint32($len))
                        }
                    }
                    Direction::Import { .. } => {
                        let offset = offset.format_term(IMPORT_PTRSIZE_EXPR, false);
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint32Le(uint32($ptr) + uint32($offset), uint32($len))
                        }
                    }
                }
            }
            Instruction::PointerStore { offset } => {
                let value = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export { .. } => {
                        let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint32Le(uint32($ptr)+uint32($offset), uint32($value))
                        }
                    }
                    Direction::Import { .. } => {
                        let offset = offset.format_term(IMPORT_PTRSIZE_EXPR, false);
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint32Le(uint32($ptr)+uint32($offset), uint32($value))
                        }
                    }
                }
            }
            Instruction::ResultLower {
                result:
                    Result_ {
                        ok: Some(_),
                        err: Some(Type::String),
                    },
                ..
            } => {
                let (err_block, _) = self.pop_block();
                let (ok_block, _) = self.pop_block();
                let operand = &operands[0];
                let (ok, err) = match operand {
                    Operand::Literal(_) => {
                        panic!("impossible: expected Operand::MultiValue but got Operand::Literal")
                    }
                    Operand::SingleValue(_) => panic!(
                        "impossible: expected Operand::MultiValue but got Operand::SingleValue"
                    ),
                    Operand::DoubleValue(ok, err) => (ok, err),
                    Operand::MultiValue(_) => panic!(
                        "impossible: expected Operand::DoubleValue but got Operand::MultiValue"
                    ),
                };
                quote_in! { self.body =>
                    $['\r']
                    if $err != nil {
                        variantPayload := $err.Error()
                        $err_block
                    } else {
                        variantPayload := $ok
                        $ok_block
                    }
                };
            }
            Instruction::ResultLower {
                result:
                    Result_ {
                        ok: None,
                        err: Some(Type::String),
                    },
                ..
            } => {
                let (err, _) = self.pop_block();
                let (ok, _) = self.pop_block();
                let err_result = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    if $err_result != nil {
                        variantPayload := $err_result.Error()
                        $err
                    } else {
                        $ok
                    }
                };
            }
            Instruction::ResultLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::OptionLift { payload, .. } => {
                let (some, some_results) = self.blocks.pop().unwrap();
                let (none, _) = self.blocks.pop().unwrap();
                let some_result = &some_results[0];

                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let ok = &format!("ok{tmp}");
                let typ = resolve_type(payload, resolve);
                let op = &operands[0];

                quote_in! { self.body =>
                    $['\r']
                    var $result $typ
                    var $ok bool
                    if $op == 0 {
                        $none
                        $ok = false
                    } else {
                        $some
                        $ok = true
                        $result = $some_result
                    }
                };

                results.push(Operand::DoubleValue(result.into(), ok.into()));
            }
            Instruction::OptionLower {
                results: result_types,
                ..
            } => {
                let (mut some_block, some_results) = self.pop_block();
                let (mut none_block, none_results) = self.pop_block();

                let tmp = self.tmp();

                // If there are no result_types, then the payload will be a pointer,
                // because that's how we represent optionals in Go.
                let is_pointer = result_types.is_empty();

                let mut vars: Tokens<Go> = Tokens::new();
                for i in 0..result_types.len() {
                    let variant = &format!("variant{tmp}_{i}");
                    let typ = resolve_wasm_type(&result_types[i]);
                    results.push(Operand::SingleValue(variant.into()));

                    quote_in! { vars =>
                        $['\r']
                        var $variant $typ
                    }

                    let some_result = &some_results[i];
                    let none_result = &none_results[i];
                    quote_in! { some_block =>
                        $['\r']
                        $variant = $some_result
                    };
                    quote_in! { none_block =>
                        $['\r']
                        $variant = $none_result
                    };
                }

                let operand = &operands[0];
                match operand {
                    Operand::Literal(_) => {
                        panic!("impossible: expected Operand::MultiValue but got Operand::Literal")
                    }
                    Operand::SingleValue(value) => {
                        quote_in! { self.body =>
                            $['\r']
                            $vars
                            if $REFLECT_VALUE_OF($value).IsZero() {
                                $none_block
                            } else {
                                variantPayload := $(if is_pointer => *)$value
                                $some_block
                            }
                        };
                    }
                    Operand::MultiValue(_) => {
                        panic!(
                            "impossible: expected Operand::DoubleValue but got Operand::MultiValue"
                        )
                    }
                    Operand::DoubleValue(value, ok) => {
                        quote_in! { self.body =>
                            $['\r']
                            if $ok {
                                variantPayload := $value
                                $some_block
                            } else {
                                $none_block
                            }
                        };
                    }
                };
            }
            Instruction::RecordLower { record, .. } => {
                let tmp = self.tmp();
                let operand = &operands[0];
                for field in record.fields.iter() {
                    let struct_field = GoIdentifier::public(&field.name);
                    let var = &GoIdentifier::local(format!("{}{tmp}", &field.name));
                    quote_in! { self.body =>
                        $['\r']
                        $var := $operand.$struct_field
                    }
                    results.push(Operand::SingleValue(var.into()))
                }
            }
            Instruction::RecordLift { record, name, .. } => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");

                // Generate pointer conversion code for optional fields
                let converted_operands: Vec<_> = record
                    .fields
                    .iter()
                    .zip(operands)
                    .enumerate()
                    .map(|(i, (field, op))| {
                        let field_type = match resolve_type(&field.ty, resolve) {
                            GoType::ValueOrOk(inner_type) => GoType::Pointer(inner_type),
                            other => other,
                        };
                        let op_clone = op.clone();
                        quote_in! { self.body =>
                            $['\r']
                        };
                        match (&field_type, &op_clone) {
                            (GoType::Pointer(inner_type), Operand::DoubleValue(val, ok)) => {
                                quote_in! { self.body =>
                                    $['\r']
                                };
                                let ptr_var_name = format!("ptr{tmp}x{i}");
                                quote_in! { self.body =>
                                    $['\r']
                                };
                                let val_ident = GoIdentifier::local(val);
                                let ok_ident = GoIdentifier::local(ok);
                                let ptr_var_ident = &GoIdentifier::local(&ptr_var_name);
                                quote_in! { self.body =>
                                    $['\r']
                                    var $(ptr_var_ident) *$(inner_type.as_ref())
                                    if $(&ok_ident) {
                                        $(ptr_var_ident) = &$(&val_ident)
                                    } else {
                                        $(ptr_var_ident) = nil
                                    }
                                };
                                Operand::SingleValue(ptr_var_name)
                            }
                            _ => {
                                quote_in! { self.body =>
                                    $['\r']
                                };
                                op_clone
                            }
                        }
                    })
                    .collect();

                let fields = record
                    .fields
                    .iter()
                    .zip(&converted_operands)
                    .map(|(field, op)| (GoIdentifier::public(&field.name), op));

                quote_in! {self.body =>
                    $['\r']
                    $value := $(GoIdentifier::public(*name)){
                        $(for (name, op) in fields join ($['\r']) => $name: $op,)
                    }
                };
                results.push(Operand::SingleValue(value.into()))
            }
            Instruction::IterElem { .. } => results.push(Operand::SingleValue(iter_element.into())),
            Instruction::IterBasePointer => results.push(Operand::SingleValue(iter_base.into())),
            Instruction::ListLower { realloc: None, .. } => {
                todo!("implement instruction: {inst:?}")
            }
            Instruction::ListLower {
                element,
                realloc: Some(realloc_name),
            } => {
                let (body, _) = self.pop_block();
                let tmp = self.tmp();
                let vec = &format!("vec{tmp}");
                let result = &format!("result{tmp}");
                let err = &format!("err{tmp}");
                let default = &format!("default{tmp}");
                let ptr = &format!("ptr{tmp}");
                let len = &format!("len{tmp}");
                let operand = &operands[0];
                let size = &self
                    .sizes
                    .size(element)
                    .format_term(EXPORT_PTRSIZE_EXPR, false);
                let align = self.sizes.align(element).format(EXPORT_PTRSIZE_EXPR);

                quote_in! { self.body =>
                    $['\r']
                    $vec := $operand
                    $len := uint64(len($vec))
                    $result, $err := i.module.ExportedFunction($(quoted(*realloc_name))).Call(ctx, 0, 0, uint64($align), $len * uint64($size))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if $err != nil {
                                var $default $(typ.as_ref())
                                return $default, $err
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if $err != nil {
                                return $err
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if $err != nil {
                                panic($err)
                            }
                        }
                    })
                    $ptr := $result[0]
                    for idx := uint64(0); idx < $len; idx++ {
                        $iter_element := $vec[idx]
                        $iter_base := uint32($ptr + uint64(idx) * uint64($size))
                        $body
                    }
                };
                results.push(Operand::SingleValue(ptr.into()));
                results.push(Operand::SingleValue(len.into()));
            }
            Instruction::ListLift { element, .. } => {
                let (body, body_results) = self.pop_block();
                let tmp = self.tmp();
                let size = self
                    .sizes
                    .size(element)
                    .format_term(EXPORT_PTRSIZE_EXPR, false);
                let len = &format!("len{tmp}");
                let base = &format!("base{tmp}");
                let result = &format!("result{tmp}");
                let idx = &format!("idx{tmp}");

                let base_operand = &operands[0];
                let len_operand = &operands[1];
                let body_result = &body_results[0];

                let typ = resolve_type(element, resolve);

                quote_in! { self.body =>
                    $['\r']
                    $base := $base_operand
                    $len := $len_operand
                    $result := make([]$typ, $len)
                    for $idx := uint64(0); $idx < $len; $idx++ {
                        base := $base + $idx * uint64($size)
                        $body
                        $result[$idx] = $body_result
                    }
                }
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::VariantLower {
                variant,
                results: result_types,
                ..
            } => {
                let blocks = self
                    .blocks
                    .drain(self.blocks.len() - variant.cases.len()..)
                    .collect::<Vec<_>>();
                let tmp = self.tmp();
                let value = &operands[0];
                let default = &format!("default{tmp}");

                for (i, _typ) in result_types.iter().enumerate() {
                    let variant_item = &format!("variant{tmp}_{i}");
                    // TODO: Use uint64 for all variant variables since they hold encoded WebAssembly values
                    let typ = GoType::Uint64;
                    quote_in! { self.body =>
                        $['\r']
                        var $variant_item $typ
                    }
                    results.push(Operand::SingleValue(variant_item.into()));
                }

                // Find the parent variant's name by comparing case names
                let variant_name = resolve.types.iter().find_map(|(_, type_def)| {
                    if let TypeDefKind::Variant(v) = &type_def.kind {
                        // Compare case names to identify the matching variant
                        if v.cases.len() == variant.cases.len()
                            && v.cases
                                .iter()
                                .zip(variant.cases.iter())
                                .all(|(a, b)| a.name == b.name)
                        {
                            type_def.name.as_ref()
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                });

                let variant_name = match variant_name {
                    Some(name) => name,
                    None => {
                        eprintln!("Warning: Could not find variant name, using 'Unknown'");
                        "Unknown"
                    }
                };

                // Pre-generate all prefixed case names to handle string lifetimes
                let case_names: Vec<String> = variant
                    .cases
                    .iter()
                    .map(|case| {
                        let capitalized_case = case
                            .name
                            .replace("-", " ")
                            .split_whitespace()
                            .map(|word| {
                                let mut chars = word.chars();
                                match chars.next() {
                                    None => String::new(),
                                    Some(first) => {
                                        first.to_uppercase().collect::<String>() + chars.as_str()
                                    }
                                }
                            })
                            .collect::<String>();
                        format!("{}{}", variant_name, capitalized_case)
                    })
                    .collect();

                let mut cases: Tokens<Go> = Tokens::new();
                for ((_case, (block, block_results)), case_name) in
                    variant.cases.iter().zip(blocks).zip(case_names.iter())
                {
                    let mut assignments: Tokens<Go> = Tokens::new();
                    for (i, result) in block_results.iter().enumerate() {
                        let variant_item = &format!("variant{tmp}_{i}");
                        quote_in! { assignments =>
                            $['\r']
                            $variant_item = $result
                        };
                    }

                    let name = GoIdentifier::public(case_name.clone());
                    quote_in! { cases =>
                        $['\r']
                        case $name:
                            $block
                            $assignments
                    }
                }

                quote_in! { self.body =>
                    $['\r']
                    switch variantPayload := $value.(type) {
                        $cases
                        default:
                            $(match &self.result {
                                GoResult::Anon(GoType::ValueOrError(typ)) => {
                                    var $default $(typ.as_ref())
                                    return $default, $ERRORS_NEW("invalid variant type provided")
                                }
                                GoResult::Anon(GoType::Error) => {
                                    return $ERRORS_NEW("invalid variant type provided")
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    panic($ERRORS_NEW("invalid variant type provided"))
                                }
                            })
                    }
                }
            }
            Instruction::EnumLower { enum_, .. } => {
                let value = &operands[0];
                let tmp = self.tmp();
                let enum_tmp = &format!("enum{tmp}");

                let mut cases: Tokens<Go> = Tokens::new();
                for (i, case) in enum_.cases.iter().enumerate() {
                    let case_name = GoIdentifier::public(case.name.clone());
                    quote_in! { cases =>
                        $['\r']
                        case $case_name:
                            $enum_tmp = $i
                    };
                }

                quote_in! { self.body =>
                    $['\r']
                    var $enum_tmp uint32
                    switch $value {
                    $cases
                    default:
                        panic($ERRORS_NEW("invalid enum type provided"))
                    }
                };

                results.push(Operand::SingleValue(enum_tmp.to_string()));
            }
            Instruction::Bitcasts { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load8S { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load16U { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load16S { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I64Load { offset } => {
                let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint64Le(uint32($operand) + uint32($offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read i64 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read i64 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read i64 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::F32Load { offset } => {
                let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint64Le(uint32($operand) + uint32($offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read f64 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::F64Load { offset } => {
                let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint64Le(uint32($operand) + uint32($offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $ERRORS_NEW("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $ERRORS_NEW("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($ERRORS_NEW("failed to read f64 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32Store16 { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I64Store { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::F32Store { offset } => {
                let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                let tag = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint64Le(uint32($ptr)+uint32($offset), $tag)
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint64Le(uint32($ptr)+uint32($offset), $tag)
                        }
                    }
                }
            }
            Instruction::F64Store { offset } => {
                let offset = offset.format_term(EXPORT_PTRSIZE_EXPR, false);
                let tag = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint64Le(uint32($ptr)+uint32($offset), $tag)
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint64Le(uint32($ptr)+uint32($offset), $tag)
                        }
                    }
                }
            }
            Instruction::I32FromChar => todo!("implement instruction: {inst:?}"),
            Instruction::I64FromU64 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := int64($operand)
                }
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I64FromS64 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := $operand
                }
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromS32 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := $WAZERO_API_ENCODE_I32($operand)
                }
                results.push(Operand::SingleValue(value))
            }
            // All of these values should fit in Go's `int32` type which allows a safe cast
            Instruction::I32FromU16
            | Instruction::I32FromS16
            | Instruction::I32FromU8
            | Instruction::I32FromS8 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := $WAZERO_API_ENCODE_I32(int32($operand))
                }
                results.push(Operand::SingleValue(value))
            }
            Instruction::CoreF32FromF32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_ENCODE_F32(float32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::CoreF64FromF64 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_ENCODE_F64(float64($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            // TODO: Validate the Go cast truncates the upper bits in the I32
            Instruction::S8FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := int8($WAZERO_API_DECODE_I32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            // TODO: Validate the Go cast truncates the upper bits in the I32
            Instruction::U8FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := uint8($WAZERO_API_DECODE_U32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            // TODO: Validate the Go cast truncates the upper bits in the I32
            Instruction::S16FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := int16($WAZERO_API_DECODE_I32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            // TODO: Validate the Go cast truncates the upper bits in the I32
            Instruction::U16FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := uint16($WAZERO_API_DECODE_U32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S32FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_DECODE_I32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S64FromI64 => todo!("implement instruction: {inst:?}"),
            Instruction::U64FromI64 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $(&value) := uint64($operand)
                }
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::CharFromI32 => todo!("implement instruction: {inst:?}"),
            Instruction::F32FromCoreF32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_DECODE_F32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::F64FromCoreF64 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $WAZERO_API_DECODE_F64($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::TupleLower { tuple, .. } => {
                let tmp = self.tmp();
                let operand = &operands[0];
                for (i, _) in tuple.types.iter().enumerate() {
                    let field = GoIdentifier::public(format!("f-{i}"));
                    let var = &GoIdentifier::local(format!("f-{tmp}-{i}"));
                    quote_in! { self.body =>
                        $['\r']
                        $var := $operand.$field
                    }
                    results.push(Operand::SingleValue(var.into()));
                }
            }
            Instruction::TupleLift { tuple, ty } => {
                if tuple.types.len() != operands.len() {
                    panic!(
                        "impossible: expected {} operands but got {}",
                        tuple.types.len(),
                        operands.len()
                    );
                }
                let tmp = self.tmp();
                let value = &GoIdentifier::local(format!("value{tmp}"));

                let mut ty_tokens = Tokens::new();
                if let Some(ty) = resolve
                    .types
                    .get(ty.clone())
                    .expect("failed to find tuple type definition")
                    .name
                    .as_ref()
                {
                    let ty_name = GoIdentifier::public(ty);
                    ty_name.format_into(&mut ty_tokens);
                } else {
                    ty_tokens.append(static_literal("struct{"));
                    if let Some((last, typs)) = tuple.types.split_last() {
                        for (i, typ) in typs.iter().enumerate() {
                            let go_type = resolve_type(typ, resolve);
                            let field = GoIdentifier::public(format!("f-{i}"));
                            field.format_into(&mut ty_tokens);
                            ty_tokens.space();
                            go_type.format_into(&mut ty_tokens);
                            ty_tokens.append(static_literal(";"));
                            ty_tokens.space();
                        }
                        let field = GoIdentifier::public(format!("f-{}", typs.len()));
                        field.format_into(&mut ty_tokens);
                        let go_type = resolve_type(last, resolve);
                        ty_tokens.space();
                        ty_tokens.append(go_type);
                    }
                    ty_tokens.append(static_literal("}"));
                }
                quote_in! { self.body =>
                    $['\r']
                    var $value $ty_tokens
                }
                for (i, (operand, _)) in operands.iter().zip(&tuple.types).enumerate() {
                    let field = &GoIdentifier::public(format!("f-{i}"));
                    quote_in! { self.body =>
                        $['\r']
                        $value.$field = $operand
                    }
                }
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::FlagsLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FlagsLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::VariantLift { .. } => {
                todo!("implement instruction: {inst:?}")
            }
            Instruction::EnumLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::Malloc {
                realloc,
                size,
                align,
            } => {
                let tmp = self.tmp();
                let ptr = &format!("ptr{tmp}");
                let result = &format!("result{tmp}");
                let err = &format!("err{tmp}");
                let default = &format!("default{tmp}");
                let size = size.format_term(EXPORT_PTRSIZE_EXPR, false);
                let align = align.format(EXPORT_PTRSIZE_EXPR);

                quote_in! { self.body =>
                    $['\r']
                    $result, $err := i.module.ExportedFunction($(quoted(*realloc))).Call(ctx, 0, 0, uint64($align), uint64($size))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if $err != nil {
                                var $default $(typ.as_ref())
                                return $default, $err
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if $err != nil {
                                return $err
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if $err != nil {
                                panic($err)
                            }
                        }
                    })
                    $ptr := $result[0]
                }
                results.push(Operand::SingleValue(ptr.into()));
            }
            Instruction::HandleLower {
                handle,
                name: _,
                ty: _ty,
            } => match handle {
                // Create an `i32` from a handle.
                // For constructors, we need to store the resource in the table and return the handle.
                // For other cases, just convert to uint32.
                Handle::Own(_id) | Handle::Borrow(_id) => {
                    let tmp = self.tmp();
                    let converted = &format!("converted{tmp}");
                    let operand = &operands[0];

                    // Check if this is in a constructor context (we have resource_context and just called NewFoo)
                    match &self.direction {
                        Direction::Import {
                            resource_context, ..
                        } if resource_context.is_some() => {
                            let ctx = resource_context.as_ref().unwrap();
                            let table_var = &ctx.table_var;

                            quote_in! { self.body =>
                                $['\r']
                                $converted := uint32($table_var.Store($operand))
                            }
                        }
                        _ => {
                            quote_in! { self.body =>
                                $['\r']
                                $converted := uint32($operand)
                            }
                        }
                    }

                    results.push(Operand::SingleValue(converted.into()));
                }
            },
            Instruction::HandleLift {
                handle,
                name,
                ty: _ty,
            } => {
                // Convert an i32 from Wasm into a resource handle.
                // In the Component Model, the i32 is an index into the resource table.
                // We need to get the proper resource type name (with interface prefix).
                match handle {
                    Handle::Own(_id) | Handle::Borrow(_id) => {
                        let tmp = self.tmp();
                        let converted = &format!("converted{tmp}");
                        let operand = &operands[0];

                        // Use the properly prefixed resource type name
                        let resource_type = match &self.direction {
                            Direction::Import {
                                resource_context, ..
                            }
                            | Direction::Export {
                                resource_context, ..
                            } if resource_context.is_some() => {
                                let ctx = resource_context.as_ref().unwrap();
                                // Use kebab-case format for proper identifier conversion
                                GoIdentifier::private(&format!(
                                    "{}-{}-handle",
                                    ctx.interface_name, ctx.resource_name
                                ))
                            }
                            _ => GoIdentifier::public(*name),
                        };

                        quote_in! { self.body =>
                            $['\r']
                            $converted := $resource_type($operand)
                        }

                        results.push(Operand::SingleValue(converted.into()));
                    }
                }
            }
            Instruction::ListCanonLower { .. } | Instruction::ListCanonLift { .. } => {
                unimplemented!("gravity doesn't represent lists as Canonical")
            }
            Instruction::GuestDeallocateString
            | Instruction::GuestDeallocate { .. }
            | Instruction::GuestDeallocateList { .. }
            | Instruction::GuestDeallocateVariant { .. } => {
                unimplemented!("gravity doesn't generate the Guest code")
            }
            Instruction::FutureLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FutureLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::StreamLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::StreamLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::ErrorContextLower => todo!("implement instruction: {inst:?}"),
            Instruction::ErrorContextLift => todo!("implement instruction: {inst:?}"),
            Instruction::AsyncTaskReturn { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::DropHandle { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::Flush { amt } => {
                for op in operands.iter().take(*amt) {
                    results.push(op.clone());
                }
            }
        }
    }

    fn return_pointer(&mut self, _size: ArchitectureSize, _align: Alignment) -> Self::Operand {
        unimplemented!("return_pointer")
    }

    fn push_block(&mut self) {
        let prev = mem::replace(&mut self.body, Tokens::new());
        self.block_storage.push(prev);
    }

    fn finish_block(&mut self, operands: &mut Vec<Self::Operand>) {
        let to_restore = self.block_storage.pop().expect("should have body");
        let src = mem::replace(&mut self.body, to_restore);
        self.blocks.push((src, mem::take(operands)));
    }

    fn sizes(&self) -> &SizeAlign {
        self.sizes
    }

    fn is_list_canonical(&self, _resolve: &Resolve, _element: &Type) -> bool {
        // Go slices are never directly in the Wasm Memory, so they are never "canonical"
        false
    }
}
