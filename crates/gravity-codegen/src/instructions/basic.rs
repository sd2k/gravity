use super::InstructionHandler;
use crate::context::GenerationContext;
use anyhow::Result;
use genco::prelude::*;
use wit_bindgen_core::abi::Instruction;
use wit_component::DecodedWasm;

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
        _decoded: &DecodedWasm,
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
            Instruction::ConstZero { tys: _ } => {
                // For now, assuming single zero value
                let operand = gravity_go::Operand::Literal("0".to_string());
                context.push_operand(operand);
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
