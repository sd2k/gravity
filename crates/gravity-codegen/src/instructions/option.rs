use super::InstructionHandler;
use crate::context::GenerationContext;
use anyhow::Result;
use wit_bindgen_core::abi::Instruction;
use wit_component::DecodedWasm;

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
        _decoded: &DecodedWasm,
    ) -> Result<()> {
        // TODO: Implement option handling
        Ok(())
    }
}
