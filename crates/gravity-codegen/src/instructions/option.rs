use super::InstructionHandler;
use crate::context::GenerationContext;
use anyhow::Result;
use wit_bindgen_core::abi::Instruction;
use wit_bindgen_core::wit_parser::Resolve;

/// Handler for WebAssembly option instructions.
///
/// Processes option-related instructions including lift, lower,
/// and Option type conversions.
pub struct OptionInstructionHandler;

impl InstructionHandler for OptionInstructionHandler {
    fn can_handle(&self, instruction: &Instruction) -> bool {
        matches!(
            instruction,
            Instruction::OptionLift { .. } | Instruction::OptionLower { .. }
        )
    }

    fn handle(
        &self,
        _instruction: &Instruction,
        _context: &mut GenerationContext,
        _resolve: &Resolve,
    ) -> Result<()> {
        // TODO: Implement option handling
        Ok(())
    }
}
