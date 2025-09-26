use super::InstructionHandler;
use crate::context::GenerationContext;
use anyhow::Result;
use wit_bindgen_core::abi::Instruction;
use wit_bindgen_core::wit_parser::Resolve;

/// Handler for enum-related instructions.
///
/// Enums in the component model are simple discriminated unions without payloads,
/// represented as integers in the ABI.
pub struct EnumInstructionHandler;

impl InstructionHandler for EnumInstructionHandler {
    fn can_handle(&self, instruction: &Instruction) -> bool {
        matches!(
            instruction,
            Instruction::EnumLift { .. } | Instruction::EnumLower { .. }
        )
    }

    fn handle(
        &self,
        _instruction: &Instruction,
        _context: &mut GenerationContext,
        _resolve: &Resolve,
    ) -> Result<()> {
        // TODO: Implement enum handling
        // Enums are simpler than variants - they're just integers
        // EnumLift: Convert from i32 to enum type
        // EnumLower: Convert from enum type to i32
        Ok(())
    }
}
