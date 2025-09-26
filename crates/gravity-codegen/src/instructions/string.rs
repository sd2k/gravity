use super::InstructionHandler;
use crate::context::GenerationContext;
use anyhow::Result;
use wit_bindgen_core::abi::Instruction;
use wit_component::DecodedWasm;

pub struct StringInstructionHandler;

impl InstructionHandler for StringInstructionHandler {
    fn can_handle(&self, instruction: &Instruction) -> bool {
        matches!(
            instruction,
            Instruction::StringLift { .. } | Instruction::StringLower { .. }
        )
    }

    fn handle(
        &self,
        _instruction: &Instruction,
        _context: &mut GenerationContext,
        _decoded: &DecodedWasm,
    ) -> Result<()> {
        // TODO: Implement string handling
        Ok(())
    }
}
