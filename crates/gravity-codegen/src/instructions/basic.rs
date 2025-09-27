use super::InstructionHandler;
use crate::context::GenerationContext;
use anyhow::Result;
use genco::prelude::*;
use wit_bindgen_core::abi::Instruction;
use wit_bindgen_core::wit_parser::Resolve;

/// Handler for basic WebAssembly instructions.
///
/// Processes fundamental instructions such as constants, argument retrieval,
/// stack manipulation, and basic arithmetic operations.
pub struct BasicInstructionHandler;

impl InstructionHandler for BasicInstructionHandler {
    fn can_handle(&self, instruction: &Instruction) -> bool {
        matches!(
            instruction,
            Instruction::GetArg { .. }
                | Instruction::I32Const { .. }
                | Instruction::I32FromBool
                | Instruction::BoolFromI32
                | Instruction::I32FromU32
                | Instruction::U32FromI32
                | Instruction::I32FromS32
                | Instruction::S32FromI32
                | Instruction::I64FromU64
                | Instruction::U64FromI64
                | Instruction::I64FromS64
                | Instruction::S64FromI64
                | Instruction::I32FromU8
                | Instruction::U8FromI32
                | Instruction::I32FromS8
                | Instruction::S8FromI32
                | Instruction::I32FromU16
                | Instruction::U16FromI32
                | Instruction::I32FromS16
                | Instruction::S16FromI32
                | Instruction::I32Load8U { .. }
                | Instruction::CallWasm { .. }
                | Instruction::ConstZero { .. }
        )
    }

    fn handle(
        &self,
        instruction: &Instruction,
        context: &mut GenerationContext,
        _resolve: &Resolve,
    ) -> Result<()> {
        match instruction {
            Instruction::GetArg { nth } => {
                let operand = gravity_go::Operand::SingleValue(format!("arg{}", nth));
                context.push_operand(operand);
            }
            Instruction::I32Const { val } => {
                let operand = gravity_go::Operand::Literal(val.to_string());
                context.push_operand(operand);
            }
            Instruction::ConstZero { tys } => {
                for _ in tys.iter() {
                    let operand = gravity_go::Operand::Literal("0".to_string());
                    context.push_operand(operand);
                }
            }
            Instruction::I32FromBool => {
                let operands = context.pop_operands(1);
                let tmp = context.tmp();
                let var = format!("boolToI32{}", tmp);
                quote_in! { context.body =>
                    $['\r']
                    var $(&var) int32
                    if $(&operands[0]) {
                        $(&var) = 1
                    }
                }
                context.push_operand(gravity_go::Operand::SingleValue(var));
            }
            Instruction::BoolFromI32 => {
                let operands = context.pop_operands(1);
                let tmp = context.tmp();
                let var = format!("i32ToBool{}", tmp);
                quote_in! { context.body =>
                    $['\r']
                    $(&var) := $(&operands[0]) != 0
                }
                context.push_operand(gravity_go::Operand::SingleValue(var));
            }
            Instruction::I32FromU32
            | Instruction::U32FromI32
            | Instruction::I32FromS32
            | Instruction::S32FromI32 => {
                // These are no-ops in Go as int32 and uint32 can be freely converted
                let operands = context.pop_operands(1);
                let tmp = context.tmp();
                let var = format!("converted{}", tmp);
                let target_type = match instruction {
                    Instruction::I32FromU32 | Instruction::I32FromS32 => "int32",
                    Instruction::U32FromI32 => "uint32",
                    Instruction::S32FromI32 => "int32",
                    _ => unreachable!(),
                };
                quote_in! { context.body =>
                    $['\r']
                    $(&var) := $(target_type)($(&operands[0]))
                }
                context.push_operand(gravity_go::Operand::SingleValue(var));
            }
            Instruction::I64FromU64
            | Instruction::U64FromI64
            | Instruction::I64FromS64
            | Instruction::S64FromI64 => {
                let operands = context.pop_operands(1);
                let tmp = context.tmp();
                let var = format!("converted{}", tmp);
                let target_type = match instruction {
                    Instruction::I64FromU64 | Instruction::I64FromS64 => "int64",
                    Instruction::U64FromI64 => "uint64",
                    Instruction::S64FromI64 => "int64",
                    _ => unreachable!(),
                };
                quote_in! { context.body =>
                    $['\r']
                    $(&var) := $(target_type)($(&operands[0]))
                }
                context.push_operand(gravity_go::Operand::SingleValue(var));
            }

            Instruction::I32FromU8
            | Instruction::U8FromI32
            | Instruction::I32FromS8
            | Instruction::S8FromI32 => {
                let operands = context.pop_operands(1);
                let tmp = context.tmp();
                let var = format!("converted{}", tmp);
                let target_type = match instruction {
                    Instruction::I32FromU8 | Instruction::I32FromS8 => "int32",
                    Instruction::U8FromI32 => "uint8",
                    Instruction::S8FromI32 => "int8",
                    _ => unreachable!(),
                };
                quote_in! { context.body =>
                    $['\r']
                    $(&var) := $(target_type)($(&operands[0]))
                }
                context.push_operand(gravity_go::Operand::SingleValue(var));
            }
            Instruction::I32FromU16
            | Instruction::U16FromI32
            | Instruction::I32FromS16
            | Instruction::S16FromI32 => {
                let operands = context.pop_operands(1);
                let tmp = context.tmp();
                let var = format!("converted{}", tmp);
                let target_type = match instruction {
                    Instruction::I32FromU16 | Instruction::I32FromS16 => "int32",
                    Instruction::U16FromI32 => "uint16",
                    Instruction::S16FromI32 => "int16",
                    _ => unreachable!(),
                };
                quote_in! { context.body =>
                    $['\r']
                    $(&var) := $(target_type)($(&operands[0]))
                }
                context.push_operand(gravity_go::Operand::SingleValue(var));
            }
            Instruction::I32Load8U { offset } => {
                let operands = context.pop_operands(1);
                let tmp = context.tmp();
                let value = format!("value{}", tmp);
                let ok = format!("ok{}", tmp);
                let offset_val = offset.size_wasm32();
                quote_in! { context.body =>
                    $['\r']
                    $(&value), $(&ok) := i.module.Memory().ReadByte(uint32($(&operands[0]) + $(offset_val)))
                    if !$(&ok) {
                        // TODO: Handle error based on return type
                        panic("failed to read byte from memory")
                    }
                }
                context.push_operand(gravity_go::Operand::SingleValue(value));
            }
            Instruction::CallWasm { name, .. } => {
                // Handle function calls
                let _func_name = name.replace('.', "_");
                quote_in! { context.body =>
                    $['\r']
                    // Call to $(func_name) - implementation depends on function signature
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wit_bindgen_core::abi::{Instruction, WasmType};
    use wit_bindgen_core::wit_parser::{ArchitectureSize, Resolve};

    #[test]
    fn test_get_arg() {
        let handler = BasicInstructionHandler;
        let mut context = GenerationContext::new();
        let resolve = Resolve::default();

        let instruction = Instruction::GetArg { nth: 0 };
        assert!(handler.can_handle(&instruction));

        handler
            .handle(&instruction, &mut context, &resolve)
            .unwrap();
        assert_eq!(context.operands.len(), 1);
        assert_eq!(
            context.operands[0],
            gravity_go::Operand::SingleValue("arg0".to_string())
        );

        let instruction = Instruction::GetArg { nth: 5 };
        handler
            .handle(&instruction, &mut context, &resolve)
            .unwrap();
        assert_eq!(context.operands.len(), 2);
        assert_eq!(
            context.operands[1],
            gravity_go::Operand::SingleValue("arg5".to_string())
        );
    }

    #[test]
    fn test_i32_const() {
        let handler = BasicInstructionHandler;
        let mut context = GenerationContext::new();
        let resolve = Resolve::default();

        let instruction = Instruction::I32Const { val: 42 };
        assert!(handler.can_handle(&instruction));

        handler
            .handle(&instruction, &mut context, &resolve)
            .unwrap();
        assert_eq!(context.operands.len(), 1);
        assert_eq!(
            context.operands[0],
            gravity_go::Operand::Literal("42".to_string())
        );

        let instruction = Instruction::I32Const { val: -100 };
        handler
            .handle(&instruction, &mut context, &resolve)
            .unwrap();
        assert_eq!(context.operands.len(), 2);
        assert_eq!(
            context.operands[1],
            gravity_go::Operand::Literal("-100".to_string())
        );
    }

    #[test]
    fn test_const_zero_single() {
        let handler = BasicInstructionHandler;
        let mut context = GenerationContext::new();
        let resolve = Resolve::default();

        let tys = [WasmType::I32];
        let instruction = Instruction::ConstZero { tys: &tys };
        assert!(handler.can_handle(&instruction));

        handler
            .handle(&instruction, &mut context, &resolve)
            .unwrap();
        assert_eq!(context.operands.len(), 1);
        assert_eq!(
            context.operands[0],
            gravity_go::Operand::Literal("0".to_string())
        );
    }

    #[test]
    fn test_const_zero_multiple() {
        let handler = BasicInstructionHandler;
        let mut context = GenerationContext::new();
        let resolve = Resolve::default();

        let tys = [WasmType::I32, WasmType::I64, WasmType::I32];
        let instruction = Instruction::ConstZero { tys: &tys };

        handler
            .handle(&instruction, &mut context, &resolve)
            .unwrap();
        assert_eq!(context.operands.len(), 3);
        assert_eq!(
            context.operands[0],
            gravity_go::Operand::Literal("0".to_string())
        );
        assert_eq!(
            context.operands[1],
            gravity_go::Operand::Literal("0".to_string())
        );
        assert_eq!(
            context.operands[2],
            gravity_go::Operand::Literal("0".to_string())
        );
    }

    #[test]
    fn test_bool_conversions() {
        let handler = BasicInstructionHandler;
        let mut context = GenerationContext::new();
        let resolve = Resolve::default();

        // Test I32FromBool
        context.push_operand(gravity_go::Operand::SingleValue("myBool".to_string()));
        let instruction = Instruction::I32FromBool;
        assert!(handler.can_handle(&instruction));

        handler
            .handle(&instruction, &mut context, &resolve)
            .unwrap();
        assert_eq!(context.operands.len(), 1);
        // The result should be a new variable
        match &context.operands[0] {
            gravity_go::Operand::SingleValue(name) => assert!(name.starts_with("boolToI32")),
            _ => panic!("Expected SingleValue operand"),
        }

        // Check that code was generated
        let code = context.body.to_string().unwrap();
        assert!(code.contains("if myBool"));
        assert!(code.contains("= 1"));
    }

    #[test]
    fn test_can_handle() {
        let handler = BasicInstructionHandler;

        // Should handle these
        assert!(handler.can_handle(&Instruction::GetArg { nth: 0 }));
        assert!(handler.can_handle(&Instruction::I32Const { val: 42 }));
        assert!(handler.can_handle(&Instruction::I32FromBool));
        assert!(handler.can_handle(&Instruction::BoolFromI32));
        assert!(handler.can_handle(&Instruction::I32FromU32));
        assert!(handler.can_handle(&Instruction::U32FromI32));
        assert!(handler.can_handle(&Instruction::I32FromS32));
        assert!(handler.can_handle(&Instruction::S32FromI32));
        assert!(handler.can_handle(&Instruction::ConstZero { tys: &[] }));

        // Should not handle these
        assert!(!handler.can_handle(&Instruction::StringLift));
        // TODO: Fix these once we understand the new Instruction API
        // assert!(!handler.can_handle(&Instruction::OptionLift { ... }));
        // assert!(!handler.can_handle(&Instruction::ListLift { ... }));
    }

    #[test]
    fn test_integer_conversions() {
        let handler = BasicInstructionHandler;
        let mut context = GenerationContext::new();
        let resolve = Resolve::default();

        // Test U32FromI32
        context.push_operand(gravity_go::Operand::SingleValue("myInt32".to_string()));
        let instruction = Instruction::U32FromI32;
        handler
            .handle(&instruction, &mut context, &resolve)
            .unwrap();

        assert_eq!(context.operands.len(), 1);
        match &context.operands[0] {
            gravity_go::Operand::SingleValue(name) => assert!(name.starts_with("converted")),
            _ => panic!("Expected SingleValue operand"),
        }

        let code = context.body.to_string().unwrap();
        assert!(code.contains("uint32(myInt32)"));
    }

    #[test]
    fn test_i32_load8u() {
        let handler = BasicInstructionHandler;
        let mut context = GenerationContext::new();
        let resolve = Resolve::default();

        // Push a pointer operand
        context.push_operand(gravity_go::Operand::SingleValue("ptr".to_string()));

        let offset = ArchitectureSize::new(4, 0);
        let instruction = Instruction::I32Load8U { offset };

        handler
            .handle(&instruction, &mut context, &resolve)
            .unwrap();

        assert_eq!(context.operands.len(), 1);
        match &context.operands[0] {
            gravity_go::Operand::SingleValue(name) => assert!(name.starts_with("value")),
            _ => panic!("Expected SingleValue operand"),
        }

        let code = context.body.to_string().unwrap();
        assert!(code.contains("Memory().ReadByte"));
        assert!(code.contains("ptr + 4"));
    }
}
