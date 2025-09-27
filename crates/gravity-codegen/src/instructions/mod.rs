pub mod basic;
pub mod enum_handler;
pub mod list;
pub mod option;
pub mod record;
pub mod result;
pub mod string;
pub mod variant;

use crate::context::GenerationContext;
use anyhow::Result;
use wit_bindgen_core::abi::Instruction;
use wit_bindgen_core::wit_parser::Resolve;

/// Trait for handling specific types of WASM component model instructions.
pub trait InstructionHandler {
    /// Determines if this handler can process the given instruction.
    fn can_handle(&self, instruction: &Instruction) -> bool;

    /// Processes the instruction, modifying the generation context as needed.
    fn handle(
        &self,
        instruction: &Instruction,
        context: &mut GenerationContext,
        resolve: &Resolve,
    ) -> Result<()>;
}

/// Dispatches an instruction to the appropriate handler.
///
/// This function uses a match statement to efficiently route instructions
/// to their corresponding handlers based on the instruction type.
pub fn handle_instruction(
    instruction: &Instruction,
    context: &mut GenerationContext,
    resolve: &Resolve,
) -> Result<()> {
    // Import handlers only as needed
    use basic::BasicInstructionHandler;
    use enum_handler::EnumInstructionHandler;
    use list::ListInstructionHandler;
    use option::OptionInstructionHandler;
    use record::RecordInstructionHandler;
    use result::ResultInstructionHandler;
    use string::StringInstructionHandler;
    use variant::VariantInstructionHandler;

    // Use a match statement for efficient dispatch
    match instruction {
        // Basic instructions
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
        | Instruction::ConstZero { .. } => {
            BasicInstructionHandler.handle(instruction, context, resolve)
        }

        // Option instructions
        Instruction::OptionLift { .. } | Instruction::OptionLower { .. } => {
            OptionInstructionHandler.handle(instruction, context, resolve)
        }

        // Record instructions
        Instruction::RecordLift { .. } | Instruction::RecordLower { .. } => {
            RecordInstructionHandler.handle(instruction, context, resolve)
        }

        // Result instructions
        Instruction::ResultLift { .. } | Instruction::ResultLower { .. } => {
            ResultInstructionHandler.handle(instruction, context, resolve)
        }

        // List instructions
        Instruction::ListLift { .. } | Instruction::ListLower { .. } => {
            ListInstructionHandler.handle(instruction, context, resolve)
        }

        // Variant instructions
        Instruction::VariantLift { .. } | Instruction::VariantLower { .. } => {
            VariantInstructionHandler.handle(instruction, context, resolve)
        }

        // Enum instructions
        Instruction::EnumLift { .. } | Instruction::EnumLower { .. } => {
            EnumInstructionHandler.handle(instruction, context, resolve)
        }

        // String instructions
        Instruction::StringLift { .. } | Instruction::StringLower { .. } => {
            StringInstructionHandler.handle(instruction, context, resolve)
        }

        // Any other instruction is not yet handled
        _ => anyhow::bail!("Unhandled instruction: {:?}", instruction),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::GenerationContext;
    use wit_bindgen_core::abi::Instruction;

    #[test]
    fn test_handle_basic_instruction() {
        let mut context = GenerationContext::new();
        let resolve = Resolve::default();

        // Test that basic instructions are handled
        let instruction = Instruction::GetArg { nth: 0 };
        let result = handle_instruction(&instruction, &mut context, &resolve);
        assert!(result.is_ok());
        assert_eq!(context.operands.len(), 1);
    }

    #[test]
    fn test_handle_unimplemented_instruction() {
        let mut context = GenerationContext::new();
        let resolve = Resolve::default();

        // Test that unimplemented instructions return Ok (they're stubs)
        let instruction = Instruction::StringLift;
        let result = handle_instruction(&instruction, &mut context, &resolve);
        assert!(result.is_ok()); // StringInstructionHandler is a stub that returns Ok(())
    }

    // TODO: Fix these tests after updating to new Instruction API
    // The Instruction enum has changed significantly in the latest wit-bindgen-core
}
