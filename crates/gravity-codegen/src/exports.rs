use anyhow::Result;
use genco::prelude::*;
use gravity_go::{Go, GoIdentifier, GoResult, GoType, Operand};
use heck::{ToLowerCamelCase, ToUpperCamelCase};
use wit_bindgen_core::{
    abi::{AbiVariant, Bindgen, Instruction, LiftLower},
    wit_parser::{Function, Resolve, SizeAlign, WorldItem, WorldKey},
};

use crate::context::GenerationContext;
use crate::imports::GoImports;
use crate::resolve_type;

/// Configuration for export generation
pub struct ExportConfig<'a> {
    pub world_name: &'a str,
    pub instance_name: &'a GoIdentifier<'a>,
    pub go_imports: &'a GoImports,
}

/// Generator for exported functions
pub struct ExportGenerator<'a> {
    context: &'a mut GenerationContext,
    config: ExportConfig<'a>,
    resolve: &'a Resolve,
}

impl<'a> ExportGenerator<'a> {
    pub fn new(
        context: &'a mut GenerationContext,
        config: ExportConfig<'a>,
        resolve: &'a Resolve,
    ) -> Self {
        Self {
            context,
            config,
            resolve,
        }
    }

    /// Generate all exports for a world
    pub fn generate(
        mut self,
        world_exports: &indexmap::IndexMap<WorldKey, WorldItem>,
    ) -> Result<()> {
        eprintln!(
            "DEBUG ExportGenerator::generate called with {} exports",
            world_exports.len()
        );
        for (key, world_item) in world_exports.iter() {
            eprintln!("DEBUG Processing export key: {:?}", key);
            match world_item {
                WorldItem::Function(func) => {
                    eprintln!("DEBUG Generating function export: {}", func.name);
                    self.generate_function_export(func)?;
                }
                WorldItem::Interface { .. } => {
                    eprintln!("DEBUG Skipping interface export");
                    // TODO: Handle interface exports when needed
                }
                WorldItem::Type(_) => {
                    eprintln!("DEBUG Skipping type export");
                    // Type exports are handled separately
                }
            }
        }
        Ok(())
    }

    /// Generate a single exported function as a method on the instance
    fn generate_function_export(&mut self, func: &Function) -> Result<()> {
        eprintln!("DEBUG generate_function_export: {}", func.name);
        eprintln!("  func.result: {:?}", func.result);

        // Build parameters
        let mut params: Vec<(GoIdentifier<'_>, GoType)> = Vec::with_capacity(func.params.len());
        for (name, wit_type) in func.params.iter() {
            let go_type = resolve_type(wit_type, self.resolve)?;
            match go_type {
                // We can't represent this as an argument type so we unwrap the Some type
                // TODO: Figure out a better way to handle this
                GoType::ValueOrOk(typ) => params.push((GoIdentifier::Local { name }, *typ)),
                typ => params.push((GoIdentifier::Local { name }, typ)),
            }
        }

        // Determine result type
        let result = match &func.result {
            Some(wit_type) => {
                eprintln!("  About to resolve wit_type: {:?}", wit_type);
                let go_type = resolve_type(wit_type, self.resolve)?;
                eprintln!(
                    "  Resolved: Function {} has result type: {:?} -> GoType: {:?}",
                    func.name, wit_type, go_type
                );
                GoResult::Anon(go_type)
            }
            None => GoResult::Empty,
        };

        // Generate the function body using wit_bindgen_core::abi
        let mut sizes = SizeAlign::default();
        sizes.fill(self.resolve);

        let mut func_impl = FuncImpl::new(result, sizes, self.config.go_imports);

        wit_bindgen_core::abi::call(
            self.resolve,
            AbiVariant::GuestExport,
            LiftLower::LowerArgsLiftResults,
            func,
            &mut func_impl,
            false, // async is not currently supported
        );

        // Generate the method
        let fn_name = GoIdentifier::Public {
            name: &func.name.to_upper_camel_case(),
        };
        let instance = self.config.instance_name;
        let go_imports = self.config.go_imports;

        // Create arg assignments from parameters to func args
        let arg_assignments = func_impl
            .args
            .iter()
            .zip(params.iter())
            .map(|(arg, (param, _))| (arg, param))
            .collect::<Vec<_>>();

        // Debug output temporarily removed during refactor

        quote_in! { self.context.out =>
            $['\n']
            func (i *$instance) $fn_name(
                $['\r']
                ctx $(&go_imports.context),
                $(for (name, typ) in params.iter() join ($['\r']) => $name $typ,)
            ) $(&func_impl.result) {
                $(for (arg, param) in arg_assignments join ($['\r']) => $arg := $param)
                $(&func_impl.body)
            }
        };

        Ok(())
    }
}

/// Implementation of function body generation
struct FuncImpl<'a> {
    result: GoResult,
    sizes: SizeAlign,
    go_imports: &'a GoImports,
    args: Vec<String>,
    tmp_counter: usize,
    body: Tokens<Go>,
    block_storage: Vec<Tokens<Go>>,
    blocks: Vec<(Tokens<Go>, Vec<Operand>)>,
}

impl<'a> FuncImpl<'a> {
    fn new(result: GoResult, sizes: SizeAlign, go_imports: &'a GoImports) -> Self {
        Self {
            result,
            sizes,
            go_imports,
            args: Vec::new(),
            tmp_counter: 0,
            body: Tokens::new(),
            block_storage: Vec::new(),
            blocks: Vec::new(),
        }
    }

    fn tmp(&mut self) -> usize {
        let counter = self.tmp_counter;
        self.tmp_counter += 1;
        counter
    }

    fn push_arg(&mut self, arg: &str) {
        self.args.push(arg.to_string());
    }

    fn pop_block(&mut self) -> (Tokens<Go>, Vec<Operand>) {
        self.blocks.pop().expect("should have block to pop")
    }
}

impl<'a> Bindgen for FuncImpl<'a> {
    type Operand = Operand;

    fn return_pointer(
        &mut self,
        _size: wit_bindgen_core::wit_parser::ArchitectureSize,
        _align: wit_bindgen_core::wit_parser::Alignment,
    ) -> Self::Operand {
        let tmp = self.tmp();
        let ret = &format!("ret{tmp}");
        quote_in! { self.body =>
            $['\r']
            $ret := return_pointer()
        };
        Operand::SingleValue(ret.into())
    }

    fn is_list_canonical(
        &self,
        _resolve: &Resolve,
        _element: &wit_bindgen_core::wit_parser::Type,
    ) -> bool {
        false
    }

    fn emit(
        &mut self,
        resolve: &Resolve,
        inst: &Instruction<'_>,
        operands: &mut Vec<Self::Operand>,
        results: &mut Vec<Self::Operand>,
    ) {
        // Debug output for all instructions
        eprintln!("DEBUG emit called: {:?}", std::mem::discriminant(inst));
        eprintln!("  instruction = {:?}", inst);
        eprintln!("  operands = {:?}", operands);
        eprintln!("  results before = {:?}", results);

        // Debug output for ResultLift
        if matches!(inst, Instruction::ResultLift { .. }) {
            eprintln!("DEBUG: Processing ResultLift instruction");
            eprintln!("  operands.len() = {}", operands.len());
            eprintln!("  results.len() before = {}", results.len());
        }

        match inst {
            Instruction::GetArg { nth } => {
                let arg = &format!("arg{nth}");
                self.push_arg(arg);
                results.push(Operand::SingleValue(arg.into()));
            }
            Instruction::ConstZero { tys } => {
                for _ in tys.iter() {
                    results.push(Operand::Literal("0".into()));
                }
            }
            Instruction::I32FromBool => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    var $value
                    if $operand { $value = 1 } else { $value = 0 }
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::BoolFromI32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := $operand != 0
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromU8 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := uint32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromS8 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := uint32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromU16 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := uint32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromS16 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := uint32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromU32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.EncodeU32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromS32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.EncodeI32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::U32FromI32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.DecodeU32(uint64($operand))
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::S32FromI32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.DecodeI32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::U8FromI32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := uint8(api.DecodeU32($operand))
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::S8FromI32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := int8(api.DecodeI32($operand))
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::U16FromI32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := uint16(api.DecodeU32($operand))
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::S16FromI32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := int16(api.DecodeI32($operand))
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::F32FromCoreF32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.DecodeF32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::F64FromCoreF64 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.DecodeF64($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::CoreF32FromF32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.EncodeF32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::CoreF64FromF64 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.EncodeF64(float64($operand))
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I64FromS64 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.EncodeI64($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I64FromU64 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := api.EncodeU32(uint32($operand))
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::S64FromI64 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := uint64($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::U64FromI64 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := uint64($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::CharFromI32 => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := rune($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32FromChar => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let operand = &operands[0];
                quote_in! { self.body =>
                    $['\r']
                    $value := uint32($operand)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::StringLift { .. } => {
                eprintln!("DEBUG: StringLift raw operands = {:?}", operands);
                eprintln!("DEBUG: operands.len() = {}", operands.len());
                if operands.len() >= 2 {
                    eprintln!("DEBUG: operands[0] = {:?}", operands[0]);
                    eprintln!("DEBUG: operands[1] = {:?}", operands[1]);
                }
                let ptr = &operands[0];
                let len = &operands[1];
                let tmp = self.tmp();
                let buf = &format!("buf{tmp}");
                let ok = &format!("ok{tmp}");
                let value = &format!("value{tmp}");
                eprintln!("DEBUG: StringLift assigned ptr={:?}, len={:?}", ptr, len);
                quote_in! { self.body =>
                    $['\r']
                    $(&self.go_imports.fmt)("DEBUG: StringLift ptr=%d, len=%d\n", $ptr, $len)
                    $buf, $ok := i.module.Memory().Read(uint32($ptr), uint32($len))
                    $(&self.go_imports.fmt)("DEBUG: Read result -> buf_len=%d, ok=%v\n", len($buf), $ok)
                    if !$ok {
                        panic($(&self.go_imports.errors)("failed to read bytes from memory"))
                    }
                    $value := string($buf)
                    $(&self.go_imports.fmt)("DEBUG: Final string value: \"%s\"\n", $value)
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::StringLower { .. } => {
                let str = &operands[0];
                let tmp = self.tmp();
                let ptr = &format!("ptr{tmp}");
                let len = &format!("len{tmp}");
                let err = &format!("err{tmp}");
                quote_in! { self.body =>
                    $['\r']
                    memory := i.module.Memory()
                    realloc := i.module.ExportedFunction("cabi_realloc")
                    $ptr, $len, $err := writeString(ctx, $str, memory, realloc)
                    if $err != nil {
                        panic($err)
                    }
                };
                results.push(Operand::SingleValue(ptr.into()));
                results.push(Operand::SingleValue(len.into()));
            }
            Instruction::ListLift { element, .. } => {
                let (body, body_results) = self.blocks.pop().unwrap();
                let tmp = self.tmp();
                let size = self.sizes.size(element).size_wasm32();
                let len = &format!("len{tmp}");
                let base = &format!("base{tmp}");
                let result = &format!("result{tmp}");
                let idx = &format!("idx{tmp}");

                let base_operand = &operands[0];
                let len_operand = &operands[1];
                let body_result = &body_results[0];

                // Generate proper Go type for the element using resolve_type
                let go_type = crate::resolve_type(element, resolve).unwrap();

                // For quote interpolation, we need to format the GoType appropriately
                let elem_typ_tokens = {
                    let mut tokens = genco::Tokens::<genco::lang::Go>::new();
                    go_type.format_into(&mut tokens);
                    tokens.to_string().unwrap()
                };
                let typ = &elem_typ_tokens;

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
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::ListLower { element, .. } => {
                let (body, _) = self.pop_block();
                let tmp = self.tmp();
                let vec = &format!("vec{tmp}");
                let result = &format!("result{tmp}");
                let err = &format!("err{tmp}");
                let default = &format!("default{tmp}");
                let ptr = &format!("ptr{tmp}");
                let len = &format!("len{tmp}");
                let iter_element = "e";
                let iter_base = "base";
                let operand = &operands[0];
                let size = self.sizes.size(element).size_wasm32();
                let align = self.sizes.align(element).align_wasm32();

                quote_in! { self.body =>
                    $['\r']
                    $vec := $operand
                    $len := uint64(len($vec))
                    $result, $err := i.module.ExportedFunction("cabi_realloc").Call(ctx, 0, 0, $align, $len * $size)
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
            Instruction::CallWasm { name, .. } => {
                let tmp = self.tmp();
                let raw = &format!("raw{tmp}");
                let ret = &format!("results{tmp}");
                let err = &format!("err{tmp}");
                if operands.is_empty() {
                    quote_in! { self.body =>
                        $['\r']
                        $raw, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx)
                        $(&self.go_imports.fmt)("DEBUG: CallWasm %s -> raw=%v, err=%v\n", $(quoted(*name)), $raw, $err)
                        $['\r']
                    };
                } else {
                    quote_in! { self.body =>
                        $['\r']
                        $raw, $err := i.module.ExportedFunction($(quoted(*name))).Call(ctx, $(for op in operands.iter() join (, ) => uint64($op)))
                        $(&self.go_imports.fmt)("DEBUG: CallWasm %s -> raw=%v, err=%v\n", $(quoted(*name)), $raw, $err)
                        $['\r']
                    };
                }
                quote_in! { self.body =>
                    if $err != nil {
                        panic($err)
                    }
                    $ret := $raw[0]
                    $(&self.go_imports.fmt)("DEBUG: Extracted result -> %d\n", $ret)
                };
                results.push(Operand::SingleValue(ret.into()));
            }
            Instruction::CallInterface { .. } => {
                // TODO: Implement CallInterface for exports
                let tmp = self.tmp();
                let result = &format!("result{tmp}");
                quote_in! { self.body =>
                    $['\r']
                    $result := callInterface() // TODO: CallInterface not implemented
                };
                results.push(Operand::SingleValue(result.into()));
            }
            Instruction::TupleLift { .. } => {
                // For now, just pass through the operands
                for operand in operands.iter() {
                    results.push(operand.clone());
                }
            }
            Instruction::RecordLower { record, .. } => {
                let tmp = self.tmp();
                let operand = &operands[0];
                for field in record.fields.iter() {
                    let struct_field = GoIdentifier::Public { name: &field.name };
                    let var_name = format!("{}{tmp}", &field.name);
                    let var_identifier = GoIdentifier::Private { name: &var_name };
                    quote_in! { self.body =>
                        $['\r']
                        $var_identifier := $operand.$struct_field
                    }
                    // Use the formatted version (lowerCamelCase) for the operand
                    let formatted_var_name = var_name.to_lower_camel_case();
                    results.push(Operand::SingleValue(formatted_var_name))
                }
            }
            Instruction::TupleLower { .. } => {
                for operand in operands.iter() {
                    results.push(operand.clone());
                }
            }
            Instruction::EnumLower { .. } => {
                let operand = operands.pop().unwrap();
                results.push(operand);
            }
            Instruction::EnumLift { .. } => {
                let disc = operands.pop().unwrap();
                results.push(disc);
            }

            Instruction::VariantLift { .. } => {
                let disc = operands.pop().unwrap();
                results.push(disc);
            }
            Instruction::OptionLower { .. } => {
                let some = operands.pop().unwrap();
                let is_some = operands.pop().unwrap();
                results.push(is_some);
                results.push(some);
            }
            Instruction::ResultLift { result, .. } => {
                eprintln!("DEBUG: Processing ResultLift instruction");
                eprintln!("  result type: {:?}", result);

                use wit_bindgen_core::wit_parser::{Result_, Type};

                match result {
                    Result_ {
                        ok: Some(typ),
                        err: Some(Type::String),
                    } => {
                        // Pop blocks in the order they were pushed (err first, then ok)
                        let (err_block, err_results) = self.blocks.pop().unwrap();
                        let (ok_block, ok_results) = self.blocks.pop().unwrap();

                        let tmp = self.tmp();
                        let value = &format!("value{}", tmp);
                        let err = &format!("err{}", tmp);
                        let tag = &operands[0];

                        let ok_type = crate::resolve_type(typ, resolve).unwrap();

                        // Get the operands to assign from
                        let ok_op = if ok_results.is_empty() {
                            &Operand::Literal("\"\"".into())
                        } else {
                            &ok_results[0]
                        };

                        let err_op = if err_results.is_empty() {
                            &Operand::Literal("\"\"".into())
                        } else {
                            &err_results[0]
                        };

                        quote_in! { self.body =>
                            $['\r']
                            var $value $ok_type
                            var $err error
                            $(&self.go_imports.fmt)("DEBUG: ResultLift switch discriminant = %d\n", $tag)
                            switch $tag {
                            case 0:
                                $(&self.go_imports.fmt)("DEBUG: Taking OK branch\n")
                                $ok_block
                                $value = $ok_op
                            case 1:
                                $(&self.go_imports.fmt)("DEBUG: Taking Err branch\n")
                                $err_block
                                $err = $(&self.go_imports.errors)($err_op)
                            default:
                                $err = $(&self.go_imports.errors)("invalid variant discriminant for expected")
                            }
                        };

                        results.push(Operand::MultiValue((value.clone(), err.clone())));
                    }
                    Result_ {
                        ok: None,
                        err: Some(Type::String),
                    } => {
                        // Pop blocks in the order they were pushed (err first, then ok)
                        let (err_block, err_results) = self.blocks.pop().unwrap();
                        let (ok_block, _ok_results) = self.blocks.pop().unwrap();

                        let tmp = self.tmp();
                        let err = &format!("err{}", tmp);
                        let tag = &operands[0];

                        let err_op = if err_results.is_empty() {
                            &Operand::Literal("\"\"".into())
                        } else {
                            &err_results[0]
                        };

                        quote_in! { self.body =>
                            $['\r']
                            var $err error
                            $(&self.go_imports.fmt)("DEBUG: ResultLift (no ok) switch discriminant = %d\n", $tag)
                            switch $tag {
                            case 0:
                                $(&self.go_imports.fmt)("DEBUG: Taking OK branch (no value)\n")
                                $ok_block
                            case 1:
                                $(&self.go_imports.fmt)("DEBUG: Taking Err branch (no ok value)\n")
                                $err_block
                                $err = $(&self.go_imports.errors)($err_op)
                            default:
                                $err = $(&self.go_imports.errors)("invalid variant discriminant for expected")
                            }
                        };

                        results.push(Operand::SingleValue(err.clone()));
                    }
                    _ => {
                        quote_in! { self.body =>
                            $['\r']
                            // TODO: Implement ResultLift for $result
                        };
                        results.push(Operand::SingleValue("result_not_implemented".into()));
                    }
                }

                eprintln!("  results.len() after = {}", results.len());
            }
            Instruction::ResultLower { .. } => {
                for operand in operands.iter() {
                    results.push(operand.clone());
                }
            }
            // Union instructions removed in newer versions of wit-bindgen-core
            Instruction::HandleLower { .. } | Instruction::HandleLift { .. } => {
                let handle = &operands[0];
                results.push(handle.clone());
            }
            // These instructions removed in newer versions of wit-bindgen-core
            Instruction::Bitcasts { .. } => {
                let operand = &operands[0];
                results.push(operand.clone());
            }
            Instruction::Flush { amt } => {
                eprintln!("DEBUG: Processing Flush instruction with amt = {}", amt);
                eprintln!("  operands.len() = {}", operands.len());

                for n in 0..*amt {
                    if let Some(op) = operands.get(n) {
                        results.push(op.clone());
                    } else {
                        // Push placeholder value
                        let placeholder = format!("flush_{}", n);
                        results.push(Operand::SingleValue(placeholder));
                    }
                }

                eprintln!("  results.len() after = {}", results.len());
            }
            Instruction::OptionLift { payload, .. } => {
                eprintln!(
                    "DEBUG: Processing OptionLift instruction, payload: {:?}",
                    payload
                );
                eprintln!("  operands.len() = {}", operands.len());
                eprintln!("  results.len() before = {}", results.len());
                eprintln!("  blocks.len() = {}", self.blocks.len());

                // Pop blocks in the order: some first, then none
                let (some_block, some_results) = self.blocks.pop().unwrap();
                let (none_block, _none_results) = self.blocks.pop().unwrap();

                let tmp = self.tmp();
                let result = &format!("result{}", tmp);
                let ok = &format!("ok{}", tmp);
                let op = &operands[0];

                // Generate proper Go type for the payload using resolve_type
                let go_type = crate::resolve_type(payload, resolve).unwrap();

                // For quote interpolation, we need to format the GoType appropriately
                let typ_tokens = {
                    let mut tokens = genco::Tokens::<genco::lang::Go>::new();
                    go_type.format_into(&mut tokens);
                    tokens.to_string().unwrap()
                };
                let typ = &typ_tokens;

                let some_result_op = if some_results.is_empty() {
                    &Operand::Literal("false".into()) // Default value for the type
                } else {
                    &some_results[0]
                };

                quote_in! { self.body =>
                    $['\r']
                    var $result $typ
                    var $ok bool
                    if $op == 0 {
                        $none_block
                        $ok = false
                    } else {
                        $some_block
                        $ok = true
                        $result = $some_result_op
                    }
                };

                results.push(Operand::MultiValue((result.clone(), ok.clone())));

                eprintln!("  results.len() after = {}", results.len());
            }
            Instruction::Return { amt, .. } => {
                eprintln!("DEBUG: Processing Return instruction with amt = {}", amt);
                // Return consumes operands but doesn't produce results
                if *amt > 0 {
                    let operand = &operands[0];
                    quote_in! { self.body =>
                        $['\r']
                        return $operand
                    };
                }
                // Return produces no results
            }
            Instruction::I32Load8U { offset } => {
                let ptr = &operands[0];
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                eprintln!(
                    "DEBUG: I32Load8U operand ptr = {:?}, offset = {:?}",
                    ptr, offset
                );

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        // For literal numbers like "1"
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadByte(uint32($ptr + $(offset_expr.clone())))
                    $(&self.go_imports.fmt)("DEBUG: ReadByte at ptr=%d+%s -> value=%d, ok=%v\n", $ptr, $(quoted(&offset_expr)), $value, $ok)
                    if !$ok {
                        panic($(&self.go_imports.errors)("failed to read byte from memory"))
                    }
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::PointerLoad { offset } => {
                let ptr = &operands[0];
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                eprintln!(
                    "DEBUG: PointerLoad operand ptr = {:?}, offset = {:?}",
                    ptr, offset
                );

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        // For literal numbers
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint32Le(uint32($ptr + $(offset_expr.clone())))
                    $(&self.go_imports.fmt)("DEBUG: ReadUint32Le at ptr=%d+%s -> value=%d, ok=%v\n", $ptr, $(quoted(&offset_expr)), $value, $ok)
                    if !$ok {
                        panic($(&self.go_imports.errors)("failed to read pointer from memory"))
                    }
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::LengthLoad { offset } => {
                let ptr = &operands[0];
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                eprintln!(
                    "DEBUG: LengthLoad operand ptr = {:?}, offset = {:?}",
                    ptr, offset
                );

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        // For literal numbers
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint32Le(uint32($ptr + $(offset_expr.clone())))
                    $(&self.go_imports.fmt)("DEBUG: ReadUint32Le (length) at ptr=%d+%s -> value=%d, ok=%v\n", $ptr, $(quoted(&offset_expr)), $value, $ok)
                    if !$ok {
                        panic($(&self.go_imports.errors)("failed to read length from memory"))
                    }
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::IterElem { .. } => {
                results.push(Operand::SingleValue("e".into()));
            }
            Instruction::IterBasePointer => {
                results.push(Operand::SingleValue("base".into()));
            }
            Instruction::F32Store { offset } => {
                let tag = &operands[0];
                let ptr = &operands[1];

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    i.module.Memory().WriteUint64Le($ptr + $(offset_expr), $tag)
                };
                // Store instructions don't produce results
            }
            Instruction::F64Store { offset } => {
                let tag = &operands[0];
                let ptr = &operands[1];

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    i.module.Memory().WriteUint64Le($ptr + $(offset_expr), $tag)
                };
                // Store instructions don't produce results
            }
            Instruction::I32Store { offset } => {
                let tag = &operands[0];
                let ptr = &operands[1];

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    i.module.Memory().WriteUint32Le($ptr + $(offset_expr), $tag)
                };
                // Store instructions don't produce results
            }
            Instruction::I32Store8 { offset } => {
                let tag = &operands[0];
                let ptr = &operands[1];

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    i.module.Memory().WriteByte($ptr + $(offset_expr), $tag)
                };
                // Store instructions don't produce results
            }
            Instruction::F32Load { offset } => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let operand = &operands[0];

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint64Le(uint32($operand + $(offset_expr)))
                    if !$ok {
                        panic($(&self.go_imports.errors)("failed to read f32 from memory"))
                    }
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::F64Load { offset } => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let operand = &operands[0];

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint64Le(uint32($operand + $(offset_expr)))
                    if !$ok {
                        panic($(&self.go_imports.errors)("failed to read f64 from memory"))
                    }
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I32Load { offset } => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let operand = &operands[0];

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint32Le(uint32($operand + $(offset_expr)))
                    if !$ok {
                        panic($(&self.go_imports.errors)("failed to read i32 from memory"))
                    }
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::I64Load { offset } => {
                let tmp = self.tmp();
                let value = &format!("value{tmp}");
                let ok = &format!("ok{tmp}");
                let operand = &operands[0];

                // Convert ArchitectureSize offset to Go expression
                let offset_expr = {
                    let offset_debug = format!("{:?}", offset);
                    if offset_debug == "0" {
                        "0".to_string()
                    } else if offset_debug.contains("ptrsz") {
                        if offset_debug.contains("(2*ptrsz)") {
                            "8".to_string()
                        } else {
                            offset_debug.replace("ptrsz", "4")
                        }
                    } else {
                        offset_debug
                    }
                };

                quote_in! { self.body =>
                    $['\r']
                    $value, $ok := i.module.Memory().ReadUint64Le(uint32($operand + $(offset_expr)))
                    if !$ok {
                        panic($(&self.go_imports.errors)("failed to read i64 from memory"))
                    }
                };
                results.push(Operand::SingleValue(value.into()));
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
                        let field_type = match crate::resolve_type(&field.ty, resolve).unwrap() {
                            gravity_go::GoType::ValueOrOk(inner_type) => {
                                gravity_go::GoType::Pointer(inner_type)
                            }
                            other => other,
                        };
                        let op_clone = op.clone();
                        match (&field_type, &op_clone) {
                            (
                                gravity_go::GoType::Pointer(inner_type),
                                Operand::MultiValue((val, ok)),
                            ) => {
                                let ptr_var_name = format!("ptr{tmp}x{i}");
                                let val_ident = GoIdentifier::Local { name: val };
                                let ok_ident = GoIdentifier::Local { name: ok };
                                let ptr_var_ident = GoIdentifier::Local {
                                    name: &ptr_var_name,
                                };
                                quote_in! { self.body =>
                                    $['\r']
                                    var $(&ptr_var_ident) *$(inner_type.as_ref())
                                    if $(&ok_ident) {
                                        $(&ptr_var_ident) = &$(&val_ident)
                                    } else {
                                        $(&ptr_var_ident) = nil
                                    }
                                };
                                Operand::SingleValue(ptr_var_name)
                            }
                            _ => op_clone,
                        }
                    })
                    .collect();

                let fields = record
                    .fields
                    .iter()
                    .zip(&converted_operands)
                    .map(|(field, op)| (GoIdentifier::Public { name: &field.name }, op));

                quote_in! { self.body =>
                    $['\r']
                    $value := $(GoIdentifier::Public { name }){
                        $(for (name, op) in fields join ($['\r']) => $name: $op,)
                    }
                };
                results.push(Operand::SingleValue(value.into()));
            }
            Instruction::LengthStore { offset } => {
                let offset = offset.size_wasm32();
                let len = &operands[0];
                let ptr = &operands[1];
                quote_in! { self.body =>
                    $['\r']
                    i.module.Memory().WriteUint32Le($ptr+$offset, uint32($len))
                };
            }
            Instruction::PointerStore { offset } => {
                let offset = offset.size_wasm32();
                let value = &operands[0];
                let ptr = &operands[1];
                quote_in! { self.body =>
                    $['\r']
                    i.module.Memory().WriteUint32Le($ptr+$offset, uint32($value))
                };
            }
            Instruction::VariantPayloadName => {
                results.push(Operand::SingleValue("variantPayload".into()));
            }
            Instruction::I32Const { val } => {
                results.push(Operand::Literal(val.to_string()));
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
                let tmp = self.tmp_counter;
                self.tmp_counter += 1;
                let value = &operands[0];
                let default = &format!("default{tmp}");

                for (i, _typ) in result_types.iter().enumerate() {
                    let variant_item = &format!("variant{tmp}_{i}");
                    quote_in! { self.body =>
                        $['\r']
                        var $variant_item uint64
                    };
                    results.push(Operand::SingleValue(variant_item.into()));
                }

                // Find the parent variant's name by comparing case names
                let variant_name = resolve.types.iter().find_map(|(_, type_def)| {
                    if let wit_bindgen_core::wit_parser::TypeDefKind::Variant(v) = &type_def.kind {
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
                            var $default Output
                            return $default, errors.New("invalid variant type provided")
                    }
                };
            }
            _ => {
                eprintln!("DEBUG: Unhandled instruction: {:?}", inst);
                eprintln!(
                    "DEBUG: Unhandled discriminant: {:?}",
                    std::mem::discriminant(inst)
                );
                eprintln!("DEBUG: This instruction was not implemented!");
                quote_in! { self.body =>
                    $['\r']
                    // TODO: Implement instruction $(format!("{:?}", inst))
                };
                // For unhandled instructions, pass through operands as results
                for operand in operands {
                    results.push(operand.clone());
                }
            }
        }
    }

    fn sizes(&self) -> &SizeAlign {
        &self.sizes
    }

    fn push_block(&mut self) {
        eprintln!("DEBUG: push_block called");
        let prev = std::mem::replace(&mut self.body, Tokens::new());
        self.block_storage.push(prev);
    }

    fn finish_block(&mut self, operands: &mut Vec<Self::Operand>) {
        eprintln!(
            "DEBUG: finish_block called, results.len() before = {}",
            operands.len()
        );
        let to_restore = self.block_storage.pop().expect("should have body");
        let src = std::mem::replace(&mut self.body, to_restore);
        let operands_copy = std::mem::take(operands);

        eprintln!("DEBUG: block has body and {} results", operands_copy.len());

        self.blocks.push((src, operands_copy));
        eprintln!("DEBUG: finish_block done, results.len() after = 0");
    }
}
