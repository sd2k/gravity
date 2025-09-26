use super::InstructionHandler;
use crate::context::GenerationContext;
use anyhow::Result;
use wit_bindgen_core::abi::Instruction;
use wit_component::DecodedWasm;

pub struct RecordInstructionHandler;

impl InstructionHandler for RecordInstructionHandler {
    fn can_handle(&self, instruction: &Instruction) -> bool {
        matches!(
            instruction,
            Instruction::RecordLift { .. } | Instruction::RecordLower { .. }
        )
    }

    fn handle(
        &self,
        _instruction: &Instruction,
        _context: &mut GenerationContext,
        _decoded: &DecodedWasm,
    ) -> Result<()> {
        // TODO: Implement record handling
        Ok(())
    }
}
