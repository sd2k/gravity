use super::InstructionHandler;
use crate::context::GenerationContext;
use anyhow::Result;
use wit_bindgen_core::abi::Instruction;
use wit_component::DecodedWasm;

pub struct VariantInstructionHandler;

impl InstructionHandler for VariantInstructionHandler {
    fn can_handle(&self, instruction: &Instruction) -> bool {
        matches!(
            instruction,
            Instruction::VariantLift { .. }
                | Instruction::VariantLower { .. }
                | Instruction::EnumLift { .. }
                | Instruction::EnumLower { .. }
        )
    }

    fn handle(
        &self,
        _instruction: &Instruction,
        _context: &mut GenerationContext,
        _decoded: &DecodedWasm,
    ) -> Result<()> {
        // TODO: Implement variant/enum handling
        Ok(())
    }
}
