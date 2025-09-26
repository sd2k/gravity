use std::{collections::BTreeMap, fs, mem, path::Path, process::ExitCode, str::Chars};

use clap::{Arg, ArgAction, Command};
use genco::{
    Tokens,
    lang::{Go, go},
    quote, quote_in,
    tokens::{FormatInto, ItemStr, quoted, static_literal},
};
use wit_bindgen_core::{
    abi::{AbiVariant, Bindgen, Instruction, LiftLower, WasmType},
    wit_parser::{
        Alignment, ArchitectureSize, Record, Resolve, Result_, SizeAlign, Type, TypeDef,
        TypeDefKind, WorldItem,
    },
};

struct Embed<T>(T);
impl<T> FormatInto<Go> for Embed<T>
where
    T: Into<ItemStr>,
{
    fn format_into(self, tokens: &mut Tokens<Go>) {
        // TODO(#13): Submit patch to genco that will allow aliases for go imports
        // tokens.register(go::import("embed", ""));
        tokens.push();
        tokens.append(static_literal("//go:embed"));
        tokens.space();
        tokens.append(self.0.into());
    }
}

fn go_embed<T>(comment: T) -> Embed<T>
where
    T: Into<ItemStr>,
{
    Embed(comment)
}

// Format a comment where each line is preceeded by `//`.
// Based on https://github.com/udoprog/genco/blob/1ec4869f458cf71d1d2ffef77fe051ea8058b391/src/lang/csharp/comment.rs
struct Comment<T>(T);

impl<T> FormatInto<Go> for Comment<T>
where
    T: IntoIterator,
    T::Item: Into<ItemStr>,
{
    fn format_into(self, tokens: &mut Tokens<Go>) {
        for line in self.0 {
            tokens.push();
            tokens.append(static_literal("//"));
            tokens.space();
            tokens.append(line.into());
        }
    }
}

fn comment<T>(comment: T) -> Comment<T>
where
    T: IntoIterator,
    T::Item: Into<ItemStr>,
{
    Comment(comment)
}

#[derive(Debug, Clone)]
enum GoType {
    Bool,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Int8,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    String,
    Error,
    Interface,
    Pointer(Box<GoType>),
    ValueOrOk(Box<GoType>),
    ValueOrError(Box<GoType>),
    Slice(Box<GoType>),
    // MultiReturn(Vec<GoType>),
    UserDefined(String),
    Nothing,
}

impl GoType {
    /// Returns true if this type needs post-return cleanup (cabi_post_* function)
    ///
    /// According to the Component Model Canonical ABI specification, cleanup is needed
    /// for types that allocate memory in the guest's linear memory when being returned.
    ///
    /// Types that need cleanup:
    /// - Strings: allocate memory for the string data
    /// - Lists/Slices: allocate memory for the array data
    /// - Types containing the above (recursively)
    ///
    /// Types that DON'T need cleanup:
    /// - Primitives (bool, integers, floats): passed by value
    /// - Enums: represented as integers
    ///
    /// Limitations:
    /// - For UserDefined types (records, type aliases), we can't determine here if they
    ///   contain strings/lists without the full type definition, so we're conservative
    /// - A perfect implementation would recursively check record fields, but that would
    ///   require passing the Resolve context here
    fn needs_cleanup(&self) -> bool {
        match self {
            // Primitive types don't need cleanup
            GoType::Bool
            | GoType::Uint8
            | GoType::Uint16
            | GoType::Uint32
            | GoType::Uint64
            | GoType::Int8
            | GoType::Int16
            | GoType::Int32
            | GoType::Int64
            | GoType::Float32
            | GoType::Float64 => false,

            // String and slices allocate memory and need cleanup
            GoType::String | GoType::Slice(_) => true,

            // Complex types need cleanup if their inner types do
            GoType::ValueOrOk(inner) => inner.needs_cleanup(),

            // The inner type of `Err` is always a String so it requires cleanup
            // TODO(#91): Store the error type to check both inner types.
            GoType::ValueOrError(_) => true,

            // Interfaces (variants) might need cleanup (conservative approach)
            GoType::Interface => true,

            // User-defined types (records, enums, type aliases) need cleanup if they
            // contain strings or other allocated types. Since we don't have access to
            // the type definition here, we must be conservative and assume they might.
            //
            // This means we might generate unnecessary cleanup calls for:
            // - Enums (which are just integers)
            // - Records containing only primitives
            // - Type aliases to primitives
            //
            // TODO(#92): Improve this by either:
            // 1. Passing the Resolve context to check actual type definitions
            // 2. Tracking cleanup requirements during type resolution
            // 3. Using a different representation that carries this information
            GoType::UserDefined(_) => true,

            // Error is actually Result<None, String> - strings need cleanup!
            GoType::Error => true,

            // Nothing represents no value, so no cleanup needed
            GoType::Nothing => false,

            // A pointer probably needs cleanup, not sure?
            GoType::Pointer(_) => true,
        }
    }
}

impl FormatInto<Go> for &GoType {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            GoType::Bool => tokens.append(static_literal("bool")),
            GoType::Uint8 => tokens.append(static_literal("uint8")),
            GoType::Uint16 => tokens.append(static_literal("uint16")),
            GoType::Uint32 => tokens.append(static_literal("uint32")),
            GoType::Uint64 => tokens.append(static_literal("uint64")),
            GoType::Int8 => tokens.append(static_literal("int8")),
            GoType::Int16 => tokens.append(static_literal("int16")),
            GoType::Int32 => tokens.append(static_literal("int32")),
            GoType::Int64 => tokens.append(static_literal("int64")),
            GoType::Float32 => tokens.append(static_literal("float32")),
            GoType::Float64 => tokens.append(static_literal("float64")),
            GoType::String => tokens.append(static_literal("string")),
            GoType::Error => tokens.append(static_literal("error")),
            GoType::Interface => tokens.append(static_literal("interface{}")),
            GoType::ValueOrOk(value_typ) => {
                value_typ.as_ref().format_into(tokens);
                tokens.append(static_literal(","));
                tokens.space();
                tokens.append(static_literal("bool"))
            }
            GoType::ValueOrError(value_typ) => {
                value_typ.as_ref().format_into(tokens);
                tokens.append(static_literal(","));
                tokens.space();
                tokens.append(static_literal("error"))
            }
            GoType::Slice(typ) => {
                tokens.append(static_literal("[]"));
                typ.as_ref().format_into(tokens);
            }
            // GoType::MultiReturn(typs) => {
            //     tokens.append(quote!($(for typ in typs join (, ) => $typ)))
            // }
            GoType::Pointer(typ) => {
                tokens.append(static_literal("*"));
                typ.as_ref().format_into(tokens);
            }
            GoType::UserDefined(name) => {
                let id = GoIdentifier::Public { name };
                id.format_into(tokens)
            }
            GoType::Nothing => (),
        }
    }
}

impl FormatInto<Go> for GoType {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

#[derive(Clone)]
enum GoResult {
    Empty,
    Anon(GoType),
}

impl GoResult {
    /// Returns true if this result type needs post-return cleanup
    fn needs_cleanup(&self) -> bool {
        match self {
            GoResult::Empty => false,
            GoResult::Anon(typ) => typ.needs_cleanup(),
        }
    }
}

impl FormatInto<Go> for GoResult {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}
impl FormatInto<Go> for &GoResult {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match &self {
            GoResult::Anon(typ @ GoType::ValueOrError(_) | typ @ GoType::ValueOrOk(_)) => {
                // Be cautious here as there are `(` and `)` surrounding the type
                tokens.append(quote!(($typ)))
            }
            GoResult::Anon(typ) => typ.format_into(tokens),
            GoResult::Empty => (),
        }
    }
}
enum Direction {
    Export,
    Import { interface_name: String },
}

struct Func {
    direction: Direction,
    args: Vec<String>,
    result: GoResult,
    tmp: usize,
    body: Tokens<Go>,
    block_storage: Vec<Tokens<Go>>,
    blocks: Vec<(Tokens<Go>, Vec<Operand>)>,
    sizes: SizeAlign,
}

#[derive(Clone, Copy)]
enum GoIdentifier<'a> {
    Public { name: &'a str },
    Private { name: &'a str },
    Local { name: &'a str },
}

impl<'a> GoIdentifier<'a> {
    fn chars(&self) -> Chars<'a> {
        match self {
            GoIdentifier::Public { name } => name.chars(),
            GoIdentifier::Private { name } => name.chars(),
            GoIdentifier::Local { name } => name.chars(),
        }
    }
}

impl From<GoIdentifier<'_>> for String {
    fn from(value: GoIdentifier) -> Self {
        let mut tokens: Tokens<Go> = Tokens::new();
        value.format_into(&mut tokens);
        tokens.to_string().expect("to format correctly")
    }
}

impl FormatInto<Go> for &GoIdentifier<'_> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        let mut chars = self.chars();

        // TODO(#12): Check for invalid first character

        if let GoIdentifier::Public { .. } = self {
            // https://stackoverflow.com/a/38406885
            match chars.next() {
                Some(c) => tokens.append(ItemStr::from(c.to_uppercase().to_string())),
                None => panic!("No function name"),
            };
        };

        while let Some(c) = chars.next() {
            match c {
                ' ' | '-' | '_' => {
                    if let Some(c) = chars.next() {
                        tokens.append(ItemStr::from(c.to_uppercase().to_string()));
                    }
                }
                _ => tokens.append(ItemStr::from(c.to_string())),
            }
        }
    }
}

impl FormatInto<Go> for GoIdentifier<'_> {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        (&self).format_into(tokens)
    }
}

impl Func {
    fn export(result: GoResult, sizes: SizeAlign) -> Self {
        Self {
            direction: Direction::Export,
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
        }
    }

    fn import(interface_name: String, result: GoResult, sizes: SizeAlign) -> Self {
        Self {
            direction: Direction::Import { interface_name },
            args: Vec::new(),
            result,
            tmp: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
            sizes,
        }
    }

    fn tmp(&mut self) -> usize {
        let ret = self.tmp;
        self.tmp += 1;
        ret
    }

    fn args(&self) -> &[String] {
        &self.args
    }

    fn result(&self) -> &GoResult {
        &self.result
    }

    fn push_arg(&mut self, value: &str) {
        self.args.push(value.into())
    }

    fn pop_block(&mut self) -> (Tokens<Go>, Vec<Operand>) {
        self.blocks.pop().expect("should have block to pop")
    }
}

impl FormatInto<Go> for Func {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        self.body.format_into(tokens)
    }
}

#[derive(Debug, Clone)]
enum Operand {
    Literal(String),
    SingleValue(String),
    MultiValue((String, String)),
}

impl FormatInto<Go> for &Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        match self {
            Operand::Literal(val) => tokens.append(ItemStr::from(val)),
            Operand::SingleValue(val) => tokens.append(ItemStr::from(val)),
            Operand::MultiValue((val1, val2)) => {
                tokens.append(ItemStr::from(val1));
                tokens.append(static_literal(","));
                tokens.space();
                tokens.append(ItemStr::from(val2));
            }
        }
    }
}
impl FormatInto<Go> for &mut Operand {
    fn format_into(self, tokens: &mut Tokens<Go>) {
        let op: &Operand = self;
        op.format_into(tokens)
    }
}

impl Bindgen for Func {
    type Operand = Operand;

    fn emit(
        &mut self,
        resolve: &wit_bindgen_core::wit_parser::Resolve,
        inst: &wit_bindgen_core::abi::Instruction<'_>,
        operands: &mut Vec<Self::Operand>,
        results: &mut Vec<Self::Operand>,
    ) {
        let errors_new = &go::import("errors", "New");
        let iter_element = "e";
        let iter_base = "base";

        let wazero_api_decode_i32 = &go::import("github.com/tetratelabs/wazero/api", "DecodeI32");
        let wazero_api_encode_i32 = &go::import("github.com/tetratelabs/wazero/api", "EncodeI32");
        let wazero_api_decode_u32 = &go::import("github.com/tetratelabs/wazero/api", "DecodeU32");
        let wazero_api_encode_u32 = &go::import("github.com/tetratelabs/wazero/api", "EncodeU32");
        let wazero_api_decode_f32 = &go::import("github.com/tetratelabs/wazero/api", "DecodeF32");
        let wazero_api_encode_f32 = &go::import("github.com/tetratelabs/wazero/api", "EncodeF32");
        let wazero_api_decode_f64 = &go::import("github.com/tetratelabs/wazero/api", "DecodeF64");
        let wazero_api_encode_f64 = &go::import("github.com/tetratelabs/wazero/api", "EncodeF64");

        // println!("instruction: {inst:?}, operands: {operands:?}");

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
                    Direction::Export => {
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
                // TODO(#17): Wrapping every argument in `uint64` is bad and we should instead be looking
                // at the types and converting with proper guards in place
                quote_in! { self.body =>
                    $['\r']
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            $raw, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            if $err != nil {
                                var $default $(typ.as_ref())
                                return $default, $err
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            $raw, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            if $err != nil {
                                return $err
                            }
                        }
                        GoResult::Anon(_) => {
                            $raw, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if $err != nil {
                                panic($err)
                            }
                        }
                        GoResult::Empty => {
                            _, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
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
                            if _, err := i.module.ExportedFunction($(quoted(format!("cabi_post_{name}")))).Call(ctx, $raw...); err != nil {
                                $(comment(&[
                                    "If we get an error during cleanup, something really bad is",
                                    "going on, so we panic. Also, you can't return the error from",
                                    "the `defer`"
                                ]))
                                panic($errors_new("failed to cleanup"))
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
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadByte(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read byte from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read byte from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read byte from memory"))
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
                quote_in! { self.body =>
                    $['\r']
                    $result := $wazero_api_encode_u32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::U32FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $wazero_api_decode_u32(uint64($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::PointerLoad { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let ptr = &format!("ptr{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $ptr, $ok := i.module.Memory().ReadUint32Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read pointer from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read pointer from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read pointer from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(ptr.into()));
            }
            Instruction::LengthLoad { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let len = &format!("len{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $len, $ok := i.module.Memory().ReadUint32Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read length from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read length from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read length from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(len.into()));
            }
            Instruction::I32Load { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint32Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read i32 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read i32 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read i32 from memory"))
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
                            $buf, $ok := i.module.Memory().Read($ptr, $len)
                            $(match &self.result {
                                GoResult::Anon(GoType::ValueOrError(typ)) => {
                                    if !$ok {
                                        var $default $(typ.as_ref())
                                        return $default, $errors_new("failed to read bytes from memory")
                                    }
                                }
                                GoResult::Anon(GoType::Error) => {
                                    if !$ok {
                                        return $errors_new("failed to read bytes from memory")
                                    }
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    if !$ok {
                                        panic($errors_new("failed to read bytes from memory"))
                                    }
                                }
                            })
                            $str := string($buf)
                        };
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            $buf, $ok := mod.Memory().Read($ptr, $len)
                            if !$ok {
                                panic($errors_new("failed to read bytes from memory"))
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
                        $err = $errors_new($err_op)
                    default:
                        $err = $errors_new("invalid variant discriminant for expected")
                    }
                };

                results.push(Operand::MultiValue((value.into(), err.into())));
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
                        $err = $errors_new($err_op)
                    default:
                        $err = $errors_new("invalid variant discriminant for expected")
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
                let ident = GoIdentifier::Public { name: &func.name };
                let tmp = self.tmp();
                let args = quote!($(for op in operands.iter() join (, ) => $op));
                let returns = match &func.result {
                    None => GoType::Nothing,
                    Some(typ) => resolve_type(typ, resolve),
                };
                let value = &format!("value{tmp}");
                let err = &format!("err{tmp}");
                let ok = &format!("ok{tmp}");
                match &self.direction {
                    Direction::Export { .. } => todo!("TODO(#10): handle export direction"),
                    Direction::Import { interface_name, .. } => {
                        let iface = GoIdentifier::Local {
                            name: interface_name,
                        };
                        quote_in! { self.body =>
                            $['\r']
                            $(match returns {
                                GoType::Nothing => $iface.$ident(ctx, $args),
                                GoType::Bool | GoType::Uint32 | GoType::Interface | GoType::String | GoType::UserDefined(_) => $value := $iface.$ident(ctx, $args),
                                GoType::Error => $err := $iface.$ident(ctx, $args),
                                GoType::ValueOrError(_) => {
                                    $value, $err := $iface.$ident(ctx, $args)
                                }
                                GoType::ValueOrOk(_) => {
                                    $value, $ok := $iface.$ident(ctx, $args)
                                }
                                _ => $(comment(&["TODO(#9): handle return type"]))
                            })
                        }
                    }
                }
                match returns {
                    GoType::Nothing => (),
                    GoType::Bool
                    | GoType::Uint32
                    | GoType::Interface
                    | GoType::UserDefined(_)
                    | GoType::String => {
                        results.push(Operand::SingleValue(value.into()));
                    }
                    GoType::Error => {
                        results.push(Operand::SingleValue(err.into()));
                    }
                    GoType::ValueOrError(_) => {
                        results.push(Operand::MultiValue((value.into(), err.into())));
                    }
                    GoType::ValueOrOk(_) => {
                        results.push(Operand::MultiValue((value.into(), ok.into())))
                    }
                    _ => todo!("TODO(#9): handle return type - {returns:?}"),
                }
            }
            Instruction::VariantPayloadName => {
                results.push(Operand::SingleValue("variantPayload".into()));
            }
            Instruction::I32Const { val } => results.push(Operand::Literal(val.to_string())),
            Instruction::I32Store8 { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                if let Operand::Literal(byte) = tag {
                    match &self.direction {
                        Direction::Export => {
                            quote_in! { self.body =>
                                $['\r']
                                i.module.Memory().WriteByte($ptr+$offset, $byte)
                            }
                        }
                        Direction::Import { .. } => {
                            quote_in! { self.body =>
                                $['\r']
                                mod.Memory().WriteByte($ptr+$offset, $byte)
                            }
                        }
                    }
                } else {
                    let tmp = self.tmp();
                    let byte = format!("byte{tmp}");
                    match &self.direction {
                        Direction::Export => {
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
                                    panic($errors_new("invalid int8 value encountered"))
                                }
                                i.module.Memory().WriteByte($ptr+$offset, $byte)
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
                                    panic($errors_new("invalid int8 value encountered"))
                                }
                                mod.Memory().WriteByte($ptr+$offset, $byte)
                            }
                        }
                    }
                }
            }
            Instruction::I32Store { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint32Le($ptr+$offset, $tag)
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint32Le($ptr+$offset, $tag)
                        }
                    }
                }
            }
            Instruction::LengthStore { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let len = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint32Le($ptr+$offset, uint32($len))
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint32Le($ptr+$offset, uint32($len))
                        }
                    }
                }
            }
            Instruction::PointerStore { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let value = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint32Le($ptr+$offset, uint32($value))
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint32Le($ptr+$offset, uint32($value))
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
                    Operand::MultiValue(bindings) => bindings,
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
                let typ = resolve_type(payload, resolve);
                let op = &operands[0];

                quote_in! { self.body =>
                    $['\r']
                    var $result *$typ
                    if $op == 0 {
                        $none
                        $result = nil
                    } else {
                        $some
                        $result = &$some_result
                    }
                };

                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::OptionLower {
                payload: Type::String,
                results: result_types,
                ..
            } => {
                let (mut some_block, some_results) = self.pop_block();
                let (mut none_block, none_results) = self.pop_block();

                let tmp = self.tmp();

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
                    // TODO(#7): This is a weird hack to implement `option<string>`
                    // as arguments that currently only works for strings
                    // because it checks the empty string as the zero value to
                    // consider it None
                    Operand::SingleValue(value) => {
                        quote_in! { self.body =>
                            $['\r']
                            $vars
                            if $value == "" {
                                $none_block
                            } else {
                                variantPayload := $value
                                $some_block
                            }
                        };
                    }
                    Operand::MultiValue((value, ok)) => {
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
            Instruction::OptionLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::RecordLower { record, .. } => {
                let tmp = self.tmp();
                let operand = &operands[0];
                for field in record.fields.iter() {
                    let struct_field = GoIdentifier::Public { name: &field.name };
                    let var = GoIdentifier::Local {
                        name: &format!("{}{tmp}", &field.name),
                    };
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
                let fields = record
                    .fields
                    .iter()
                    .zip(operands)
                    .map(|(field, op)| (GoIdentifier::Public { name: &field.name }, op));

                quote_in! {self.body =>
                    $['\r']
                    $value := $(GoIdentifier::Public { name }){
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
                let size = self.sizes.size(element).size_wasm32();
                let align = self.sizes.align(element).align_wasm32();

                quote_in! { self.body =>
                    $['\r']
                    $vec := $operand
                    $len := uint64(len($vec))
                    $result, $err := i.module.ExportedFunction($(quoted(*realloc_name))).Call(ctx, 0, 0, $align, $len * $size)
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
                let size = self.sizes.size(element).size_wasm32();
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
                    for $idx := uint32(0); $idx < $len; $idx++ {
                        base := $base + $idx * $size
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
                                        first.to_uppercase().collect::<String>()
                                            + &chars.collect::<String>()
                                    }
                                }
                            })
                            .collect::<String>();
                        format!("{}{}", variant_name, capitalized_case)
                    })
                    .collect();

                let mut cases: Tokens<Go> = Tokens::new();
                for ((_case, prefixed_name), (block, block_results)) in
                    variant.cases.iter().zip(&case_names).zip(blocks)
                {
                    let mut assignments: Tokens<Go> = Tokens::new();
                    for (i, result) in block_results.iter().enumerate() {
                        let variant_item = &format!("variant{tmp}_{i}");
                        quote_in! { assignments =>
                            $['\r']
                            $variant_item = $result
                        };
                    }

                    let name = GoIdentifier::Public {
                        name: prefixed_name,
                    };
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
                                    return $default, $errors_new("invalid variant type provided")
                                }
                                GoResult::Anon(GoType::Error) => {
                                    return $errors_new("invalid variant type provided")
                                }
                                GoResult::Anon(_) | GoResult::Empty => {
                                    $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                                    panic($errors_new("invalid variant type provided"))
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
                    let case_name = GoIdentifier::Public { name: &case.name };
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
                        panic($errors_new("invalid enum type provided"))
                    }
                };

                results.push(Operand::SingleValue(enum_tmp.to_string()));
            }
            Instruction::Bitcasts { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load8S { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load16U { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I32Load16S { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I64Load { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint64Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read i64 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read i64 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read i64 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::F32Load { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint64Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read f64 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::F64Load { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let default = &format!("default{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint64Le(uint32($operand + $offset))
                    $(match &self.result {
                        GoResult::Anon(GoType::ValueOrError(typ)) => {
                            if !$ok {
                                var $default $(typ.as_ref())
                                return $default, $errors_new("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(GoType::Error) => {
                            if !$ok {
                                return $errors_new("failed to read f64 from memory")
                            }
                        }
                        GoResult::Anon(_) | GoResult::Empty => {
                            $(comment(&["The return type doesn't contain an error so we panic if one is encountered"]))
                            if !$ok {
                                panic($errors_new("failed to read f64 from memory"))
                            }
                        }
                    })
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32Store16 { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::I64Store { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::F32Store { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint64Le($ptr+$offset, $tag)
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint64Le($ptr+$offset, $tag)
                        }
                    }
                }
            }
            Instruction::F64Store { offset } => {
                // TODO(#58): Support additional ArchitectureSize
                let offset = offset.size_wasm32();
                let tag = &operands[0];
                let ptr = &operands[1];
                match &self.direction {
                    Direction::Export => {
                        quote_in! { self.body =>
                            $['\r']
                            i.module.Memory().WriteUint64Le($ptr+$offset, $tag)
                        }
                    }
                    Direction::Import { .. } => {
                        quote_in! { self.body =>
                            $['\r']
                            mod.Memory().WriteUint64Le($ptr+$offset, $tag)
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
                    $(&value) := $wazero_api_encode_i32($operand)
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
                    $(&value) := $wazero_api_encode_i32(int32($operand))
                }
                results.push(Operand::SingleValue(value))
            }
            Instruction::CoreF32FromF32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $wazero_api_encode_f32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::CoreF64FromF64 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    // TODO: This float64() cast is a hack to handle custom types that wrap float64.
                    // We should properly detect the underlying type and cast appropriately for generalization.
                    $result := $wazero_api_encode_f64(float64($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S8FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $(comment(&["S8FromI32"]))
                    $['\r']
                    $result := int8($wazero_api_decode_i32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::U8FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $(comment(&["U8FromI32"]))
                    $['\r']
                    $result := uint8($wazero_api_decode_u32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S16FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := int16($wazero_api_decode_i32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::U16FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := uint16($wazero_api_decode_u32($operand))
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S32FromI32 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $wazero_api_decode_i32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::S64FromI64 => todo!("implement instruction: {inst:?}"),
            Instruction::U64FromI64 => {
                let tmp = self.tmp();
                let value = format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $(comment(&["U64FromI64"]))
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
                    $result := $wazero_api_decode_f32($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::F64FromCoreF64 => {
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $result := $wazero_api_decode_f64($operand)
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::TupleLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::TupleLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FlagsLower { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::FlagsLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::VariantLift { .. } => {
                todo!("implement instruction: {inst:?}")
            }
            Instruction::EnumLift { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::Malloc { .. } => todo!("implement instruction: {inst:?}"),
            Instruction::HandleLower { .. } | Instruction::HandleLift { .. } => {
                todo!("implement resources: {inst:?}")
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
                for n in 0..*amt {
                    results.push(operands[n].clone());
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

    fn sizes(&self) -> &wit_bindgen_core::wit_parser::SizeAlign {
        &self.sizes
    }

    fn is_list_canonical(
        &self,
        _resolve: &wit_bindgen_core::wit_parser::Resolve,
        _element: &wit_bindgen_core::wit_parser::Type,
    ) -> bool {
        // Go slices are never directly in the Wasm Memory, so they are never "canonical"
        false
    }
}

fn resolve_wasm_type(typ: &WasmType) -> GoType {
    match typ {
        WasmType::I32 => GoType::Uint32,
        WasmType::I64 => GoType::Uint64,
        WasmType::F32 => GoType::Float32,
        WasmType::F64 => GoType::Float64,
        WasmType::Pointer => GoType::Uint64,
        WasmType::PointerOrI64 => GoType::Uint64,
        WasmType::Length => GoType::Uint64,
    }
}

fn resolve_type(typ: &Type, resolve: &Resolve) -> GoType {
    match typ {
        Type::Bool => GoType::Bool,
        Type::U8 => GoType::Uint8,
        Type::U16 => GoType::Uint16,
        Type::U32 => GoType::Uint32,
        Type::U64 => GoType::Uint64,
        Type::S8 => GoType::Int8,
        Type::S16 => GoType::Int16,
        Type::S32 => GoType::Int32,
        Type::S64 => GoType::Int64,
        Type::F32 => GoType::Float32,
        Type::F64 => GoType::Float64,
        Type::Char => {
            // Is this a Go "rune"?
            todo!("TODO(#6): resolve char type")
        }
        Type::String => GoType::String,
        Type::ErrorContext => todo!("TODO(#4): implement error context conversion"),
        Type::Id(typ_id) => {
            let TypeDef { name, kind, .. } = resolve.types.get(*typ_id).unwrap();
            match kind {
                TypeDefKind::Record(Record { .. }) => {
                    let typ = name.clone().expect("record to have a name");
                    GoType::UserDefined(typ)
                }
                TypeDefKind::Resource => todo!("TODO(#5): implement resources"),
                TypeDefKind::Handle(_) => todo!("TODO(#5): implement resources"),
                TypeDefKind::Flags(_) => todo!("TODO(#4): implement flag conversion"),
                TypeDefKind::Tuple(_) => todo!("TODO(#4): implement tuple conversion"),
                // Variants are handled as an empty interfaces in type signatures; however, that
                // means they require runtime type reflection
                TypeDefKind::Variant(_) => GoType::Interface,
                TypeDefKind::Enum(_) => {
                    let typ = name.clone().expect("enum to have a name");
                    GoType::UserDefined(typ)
                }
                TypeDefKind::Option(value) => {
                    GoType::ValueOrOk(Box::new(resolve_type(value, resolve)))
                }
                TypeDefKind::Result(Result_ {
                    ok: Some(ok),
                    err: Some(Type::String),
                }) => GoType::ValueOrError(Box::new(resolve_type(ok, resolve))),
                TypeDefKind::Result(Result_ {
                    ok: Some(_),
                    err: Some(_),
                }) => {
                    todo!("TODO(#4): implement remaining result conversion")
                }
                TypeDefKind::Result(Result_ {
                    ok: Some(ok),
                    err: None,
                }) => resolve_type(ok, resolve),
                TypeDefKind::Result(Result_ {
                    ok: None,
                    err: Some(Type::String),
                }) => GoType::Error,
                TypeDefKind::Result(Result_ {
                    ok: None,
                    err: Some(_),
                }) => todo!("TODO(#4): implement remaining result conversion"),
                TypeDefKind::Result(Result_ {
                    ok: None,
                    err: None,
                }) => GoType::Nothing,
                TypeDefKind::List(typ) => GoType::Slice(Box::new(resolve_type(typ, resolve))),
                TypeDefKind::Future(_) => todo!("TODO(#4): implement future conversion"),
                TypeDefKind::Stream(_) => todo!("TODO(#4): implement stream conversion"),
                TypeDefKind::Type(_) => {
                    let typ = name.clone().expect("type alias to have a name");
                    GoType::UserDefined(typ)
                }
                TypeDefKind::FixedSizeList(_, _) => {
                    todo!("TODO(#4): implement fixed size list conversion")
                }
                TypeDefKind::Unknown => todo!("TODO(#4): implement unknown conversion"),
            }
        }
    }
}

struct Bindings {
    out: Tokens<Go>,
}

impl Bindings {
    fn new() -> Self {
        Self { out: Tokens::new() }
    }

    fn define_type(&mut self, typ_def: &TypeDef, resolve: &Resolve) {
        let TypeDef { name, kind, .. } = typ_def;
        match kind {
            TypeDefKind::Record(Record { fields }) => {
                let name = GoIdentifier::Public {
                    name: &name.clone().expect("record to have a name"),
                };
                let field_types: Vec<_> = fields
                    .iter()
                    .map(|field| {
                        let field_type = match resolve_type(&field.ty, resolve) {
                            GoType::ValueOrOk(inner_type) => GoType::Pointer(inner_type),
                            other => other,
                        };
                        (GoIdentifier::Public { name: &field.name }, field_type)
                    })
                    .collect();

                quote_in! { self.out =>
                    $['\n']
                    type $name struct {
                        $(for (name, typ) in field_types join ($['\r']) => $name $typ)
                    }
                }
            }
            TypeDefKind::Resource => todo!("TODO(#5): implement resources"),
            TypeDefKind::Handle(_) => todo!("TODO(#5): implement resources"),
            TypeDefKind::Flags(_) => todo!("TODO(#4):generate flags type definition"),
            TypeDefKind::Tuple(_) => todo!("TODO(#4):generate tuple type definition"),
            TypeDefKind::Variant(variant) => {
                let name = name.clone().expect("variant to have a name");
                let variant_interface = GoIdentifier::Public { name: &name };

                let variant_function_name = format!(
                    "is{}",
                    &name
                        .replace("-", " ")
                        .split_whitespace()
                        .map(|word| {
                            let mut chars = word.chars();
                            match chars.next() {
                                None => String::new(),
                                Some(first) => {
                                    first.to_uppercase().collect::<String>()
                                        + &chars.collect::<String>()
                                }
                            }
                        })
                        .collect::<String>()
                );
                let variant_function = GoIdentifier::Private {
                    name: &variant_function_name,
                };

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
                                        first.to_uppercase().collect::<String>()
                                            + &chars.collect::<String>()
                                    }
                                }
                            })
                            .collect::<String>();
                        format!("{}{}", &name, capitalized_case)
                    })
                    .collect();

                let cases: Vec<_> = variant
                    .cases
                    .iter()
                    .zip(&case_names)
                    .map(|(case, prefixed_name)| {
                        let case_name = GoIdentifier::Public {
                            name: prefixed_name,
                        };
                        let case_type = case.ty.as_ref().map(|ty| resolve_type(ty, resolve));
                        (case_name, case_type)
                    })
                    .collect();

                quote_in! { self.out =>
                    $['\n']
                    type $variant_interface interface {
                        $variant_function()
                    }
                    $['\n']
                };

                for (case_name, case_type) in cases {
                    if let Some(inner_type) = case_type {
                        quote_in! { self.out =>
                            type $case_name $inner_type
                            func ($case_name) $variant_function() {}
                            $['\n']
                        };
                    } else {
                        quote_in! { self.out =>
                            type $case_name struct{}
                            func ($case_name) $variant_function() {}
                            $['\n']
                        };
                    }
                }
            }
            TypeDefKind::Enum(inner) => {
                let name = name.clone().expect("enum to have a name");
                let enum_type = GoIdentifier::Private { name: &name };

                let enum_interface = GoIdentifier::Public { name: &name };

                let enum_function = GoIdentifier::Private {
                    name: &format!("is-{}", &name),
                };

                let variants = inner.cases.iter().map(|variant| GoIdentifier::Public {
                    name: &variant.name,
                });

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
                let name = GoIdentifier::Public {
                    name: &name.clone().expect("string alias to have a name"),
                };
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

// `wit_component::decode` uses `root` as an arbitrary name for the primary
// world name, see
// 1. https://github.com/bytecodealliance/wasm-tools/blob/585a0bdd8f49fc05d076effaa96e63d97f420578/crates/wit-component/src/decoding.rs#L144-L147
// 2. https://github.com/bytecodealliance/wasm-tools/issues/1315
pub const PRIMARY_WORLD_NAME: &str = "root";

fn main() -> Result<ExitCode, ()> {
    let cmd = Command::new("gravity")
        .arg(
            Arg::new("world")
                .short('w')
                .long("world")
                .help("generate host bindings for the specified world")
                .default_value(PRIMARY_WORLD_NAME),
        )
        .arg(
            Arg::new("inline-wasm")
                .long("inline-wasm")
                .help("include the WebAssembly file as hex bytes in the output code")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("file")
                .help("the WebAssembly file to process")
                .required(true),
        )
        .arg(
            Arg::new("output")
                .help("the file path where output generated code should be output")
                .short('o')
                .long("output"),
        );

    let matches = cmd.get_matches();
    let selected_world = matches
        .get_one::<String>("world")
        .expect("should have a world");
    let file = matches
        .get_one::<String>("file")
        .expect("should have a file");
    let inline_wasm = matches.get_flag("inline-wasm");
    let output = matches.get_one::<String>("output");

    // Load the file specified as the `file` arg to clap
    let wasm = match fs::read(file) {
        Ok(wasm) => wasm,
        Err(_) => {
            eprintln!("unable to read file: {file}");
            return Ok(ExitCode::FAILURE);
        }
    };

    let (module, bindgen) = wit_component::metadata::decode(&wasm)
        // If the Wasm doesn't have a custom section, None will be returned so we need to use the original
        .map(|(module, bindgen)| (module.unwrap_or(wasm), bindgen))
        .expect("file should be a valid WebAssembly module");

    let wasm_file = &format!("{}.wasm", selected_world.replace('-', "_"));

    let raw_wasm = GoIdentifier::Private {
        name: &format!("wasm-file-{selected_world}"),
    };
    let factory = GoIdentifier::Public {
        name: &format!("{selected_world}-factory"),
    };
    let new_factory = GoIdentifier::Public {
        name: &format!("new-{selected_world}-factory"),
    };
    let instance = GoIdentifier::Public {
        name: &format!("{selected_world}-instance"),
    };

    let context = &go::import("context", "Context");
    let wazero_new_runtime = &go::import("github.com/tetratelabs/wazero", "NewRuntime");
    let wazero_new_module_config = &go::import("github.com/tetratelabs/wazero", "NewModuleConfig");
    let wazero_runtime = &go::import("github.com/tetratelabs/wazero", "Runtime");
    let wazero_compiled_module = &go::import("github.com/tetratelabs/wazero", "CompiledModule");
    let wazero_api_module = &go::import("github.com/tetratelabs/wazero/api", "Module");
    let wazero_api_memory = &go::import("github.com/tetratelabs/wazero/api", "Memory");
    let wazero_api_function = &go::import("github.com/tetratelabs/wazero/api", "Function");

    let mut bindings = Bindings::new();

    if inline_wasm {
        let hex_rows = module
            .chunks(16)
            .map(|bytes| {
                quote! {
                    $(for b in bytes join ( ) => $(format!("0x{b:02x},")))
                }
            })
            .collect::<Vec<Tokens<Go>>>();

        // TODO(#16): Don't use the internal bindings.out field
        quote_in! { bindings.out =>
            var $raw_wasm = []byte{
                $(for row in hex_rows join ($['\r']) => $row)
            }
        };
    } else {
        // TODO(#16): Don't use the internal bindings.out field
        quote_in! { bindings.out =>
            import _ "embed"

            $(go_embed(wasm_file))
            var $raw_wasm []byte
        }
    }

    for (_, world) in &bindgen.resolve.worlds {
        if world.name != *selected_world {
            continue;
        }

        // TODO(#16): Don't use the internal bindings.out field
        quote_in! { bindings.out =>
            $['\n']
            type $factory struct {
                runtime $wazero_runtime
                module  $wazero_compiled_module
            }
        };

        let mut import_fns: BTreeMap<String, Tokens<Go>> = BTreeMap::new();
        let mut ifaces = Vec::new();

        for (idx, world_item) in world.imports.values().enumerate() {
            match world_item {
                WorldItem::Interface { id, .. } => {
                    let iface = &bindgen.resolve.interfaces[*id];
                    let interface_name = iface.name.clone().expect("TODO");
                    let err = &format!("err{idx}");

                    // TOOD: Can this ever be empty?
                    let mut import_module_name = String::new();
                    if let Some(package) = iface.package {
                        let pkg = &bindgen.resolve.packages[package];
                        import_module_name = format!(
                            "{}:{}/{}",
                            pkg.name.namespace, pkg.name.name, interface_name
                        )
                    }

                    let import_chain = import_fns.entry(import_module_name.clone()).or_insert(
                        quote! {
                            _, $err := wazeroRuntime.NewHostModuleBuilder($(quoted(import_module_name))).
                        },
                    );

                    for typ_id in iface.types.values() {
                        let typ_def = bindgen.resolve.types.get(*typ_id).unwrap();
                        bindings.define_type(typ_def, &bindgen.resolve);
                    }

                    let mut interface_funcs = Tokens::new();
                    for func in iface.functions.values() {
                        let mut params = Vec::with_capacity(func.params.len());
                        for (name, wit_type) in func.params.iter() {
                            let go_type = resolve_type(wit_type, &bindgen.resolve);
                            params.push((GoIdentifier::Local { name }, go_type));
                        }

                        let result = match func.result {
                            Some(wit_type) => {
                                let go_type = resolve_type(&wit_type, &bindgen.resolve);
                                GoResult::Anon(go_type)
                            }
                            None => GoResult::Empty,
                        };

                        let func_name = GoIdentifier::Public { name: &func.name };
                        quote_in! { interface_funcs =>
                            $['\r']
                            $(&func_name)(
                                ctx $context,
                                $(for (name, typ) in params join ($['\r']) => $(&name) $typ,)
                            ) $result
                        };
                    }
                    let iface_name = GoIdentifier::Public {
                        name: &format!("i-{selected_world}-{interface_name}"),
                    };
                    ifaces.push(interface_name.clone());

                    // TODO(#16): Don't use the internal bindings.out field
                    quote_in! { bindings.out =>
                        $['\n']
                        type $iface_name interface {
                            $interface_funcs
                        }
                    };

                    for func in iface.functions.values() {
                        let mut sizes = SizeAlign::default();
                        sizes.fill(&bindgen.resolve);

                        let wasm_sig = bindgen
                            .resolve
                            .wasm_signature(AbiVariant::GuestImport, func);
                        let result = if wasm_sig.results.is_empty() {
                            GoResult::Empty
                        } else {
                            // TODO: Should this instead produce the results based on the wasm_sig?
                            match &func.result {
                                Some(Type::Bool) => GoResult::Anon(GoType::Uint32),
                                Some(Type::Id(typ_id)) => {
                                    let TypeDef { kind, .. } =
                                        bindgen.resolve.types.get(*typ_id).unwrap();
                                    let go_type = match kind {
                                        TypeDefKind::Enum(_) => GoType::Uint32,
                                        _ => todo!("handle Type::Id({typ_id:?})"),
                                    };
                                    GoResult::Anon(go_type)
                                }
                                Some(wit_type) => todo!("handle {wit_type:?}"),
                                None => GoResult::Empty,
                            }
                        };

                        let mut f = Func::import(interface_name.clone(), result, sizes);
                        wit_bindgen_core::abi::call(
                            &bindgen.resolve,
                            AbiVariant::GuestImport,
                            LiftLower::LiftArgsLowerResults,
                            func,
                            &mut f,
                            // async is not currently supported
                            false,
                        );
                        let name = &func.name;

                        quote_in! { *import_chain =>
                            $['\r']
                            NewFunctionBuilder().
                            $['\r']
                            WithFunc(func(
                                ctx $context,
                                mod $wazero_api_module,
                                $(for arg in f.args() join ($['\r']) => $arg uint32,)
                            ) $(f.result()) {
                                $f
                            }).
                            $['\r']
                            Export($(quoted(name))).
                        };
                    }

                    quote_in! { *import_chain =>
                        $['\r']
                        Instantiate(ctx)
                        $['\r']
                        if $err != nil {
                            return nil, $err
                        }
                    };
                }
                WorldItem::Function(_) => (),
                WorldItem::Type(id) => {
                    let typ_def = bindgen.resolve.types.get(*id).unwrap();
                    bindings.define_type(typ_def, &bindgen.resolve);
                }
            };
        }

        // TODO(#16): Don't use the internal bindings.out field
        quote_in! { bindings.out =>
            $['\n']
            func $new_factory(
                ctx $context,
                $(for interface_name in ifaces.iter() join ($['\r']) => $(GoIdentifier::Local { name: interface_name }) $(GoIdentifier::Public {
                    name: &format!("i-{selected_world}-{interface_name}"),
                }),)
            ) (*$factory, error) {
                wazeroRuntime := $wazero_new_runtime(ctx)

                $(for import_fn in import_fns.values() join ($['\r']) => $import_fn)

                $(comment(&[
                    "Compiling the module takes a LONG time, so we want to do it once and hold",
                    "onto it with the Runtime"
                ]))
                module, err := wazeroRuntime.CompileModule(ctx, $raw_wasm)
                if err != nil {
                    return nil, err
                }

                return &$factory{wazeroRuntime, module}, nil
            }

            func (f *$factory) Instantiate(ctx $context) (*$instance, error) {
                if module, err := f.runtime.InstantiateModule(ctx, f.module, $wazero_new_module_config()); err != nil {
                    return nil, err
                } else {
                    return &$instance{module}, nil
                }
            }

            func (f *$factory) Close(ctx $context) {
                f.runtime.Close(ctx)
            }
        };

        // TODO: Only apply helpers like `writeString` if they are needed
        // TODO(#16): Don't use the internal bindings.out field
        quote_in! { bindings.out =>
            $['\n']
            type $instance struct {
                module $wazero_api_module
            }

            $(comment(&[
                "writeString will put a Go string into the Wasm memory following the Component",
                "Model calling convetions, such as allocating memory with the realloc function"
            ]))
            func writeString(
                ctx $context,
                s string,
                memory $wazero_api_memory,
                realloc $wazero_api_function,
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
                    return 1, 0, err
                }
                return uint64(ptr), uint64(len(s)), nil
            }

            func (i *$instance) Close(ctx $context) error {
                if err := i.module.Close(ctx); err != nil {
                    return err
                }

                return nil
            }
        };

        for world_item in world.exports.values() {
            match world_item {
                WorldItem::Function(func) => {
                    let mut params: Vec<(GoIdentifier<'_>, GoType)> =
                        Vec::with_capacity(func.params.len());
                    for (name, wit_type) in func.params.iter() {
                        let go_type = resolve_type(wit_type, &bindgen.resolve);
                        match go_type {
                            // We can't represent this as an argument type so we unwrap the Some type
                            // TODO: Figure out a better way to handle this
                            GoType::ValueOrOk(typ) => {
                                params.push((GoIdentifier::Local { name }, *typ))
                            }
                            typ => params.push((GoIdentifier::Local { name }, typ)),
                        }
                    }

                    let mut sizes = SizeAlign::default();
                    sizes.fill(&bindgen.resolve);

                    let result = match &func.result {
                        Some(wit_type) => {
                            let go_type = resolve_type(wit_type, &bindgen.resolve);
                            GoResult::Anon(go_type)
                        }
                        None => GoResult::Empty,
                    };

                    let mut f = Func::export(result, sizes);
                    wit_bindgen_core::abi::call(
                        &bindgen.resolve,
                        AbiVariant::GuestExport,
                        LiftLower::LowerArgsLiftResults,
                        func,
                        &mut f,
                        // async is not currently supported
                        false,
                    );

                    let arg_assignments = f
                        .args()
                        .iter()
                        .zip(params.iter())
                        .map(|(arg, (param, _))| (arg, param))
                        .collect::<Vec<(&String, &GoIdentifier)>>();

                    let fn_name = &GoIdentifier::Public { name: &func.name };
                    // TODO(#16): Don't use the internal bindings.out field
                    quote_in! { bindings.out =>
                        $['\n']
                        func (i *$instance) $fn_name(
                            $['\r']
                            ctx $context,
                            $(for (name, typ) in params.iter() join ($['\r']) => $name $typ,)
                        ) $(f.result()) {
                            $(for (arg, param) in arg_assignments join ($['\r']) => $arg := $param)
                            $f
                        }
                    };
                }
                WorldItem::Interface { .. } => (),
                WorldItem::Type(_) => (),
            }
        }
    }

    let mut w = genco::fmt::FmtWriter::new(String::new());
    let fmt = genco::fmt::Config::from_lang::<Go>().with_indentation(genco::fmt::Indentation::Tab);
    let config = go::Config::default().with_package(selected_world.replace('-', "_"));

    // TODO(#16): Don't use the internal bindings.out field
    bindings
        .out
        .format_file(&mut w.as_formatter(&fmt), &config)
        .unwrap();

    match output {
        Some(outpath) => {
            if !inline_wasm {
                let wasm_outpath = Path::new(outpath).with_file_name(wasm_file);
                match fs::write(&wasm_outpath, module) {
                    Ok(_) => (),
                    Err(_) => {
                        eprintln!("failed to create file: {}", wasm_outpath.to_string_lossy());
                        return Ok(ExitCode::FAILURE);
                    }
                }
            }
            match fs::write(outpath, w.into_inner()) {
                Ok(_) => Ok(ExitCode::SUCCESS),
                Err(_) => {
                    eprintln!("failed to create file: {outpath}");
                    Ok(ExitCode::FAILURE)
                }
            }
        }
        None => {
            println!("{}", w.into_inner());
            Ok(ExitCode::SUCCESS)
        }
    }
}
