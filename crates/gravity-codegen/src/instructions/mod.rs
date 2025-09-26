pub mod basic;
pub mod list;
pub mod option;
pub mod record;
pub mod result;
pub mod string;
pub mod variant;

use crate::context::GenerationContext;
use anyhow::Result;
use wit_bindgen_core::abi::Instruction;
use wit_component::DecodedWasm;

pub trait InstructionHandler {
    fn can_handle(&self, instruction: &Instruction) -> bool;
    fn handle(
        &self,
        instruction: &Instruction,
        context: &mut GenerationContext,
        decoded: &DecodedWasm,
    ) -> Result<()>;
}

pub fn handle_instruction(
    instruction: &Instruction,
    context: &mut GenerationContext,
    decoded: &DecodedWasm,
) -> Result<()> {
    use basic::BasicInstructionHandler;
    use list::ListInstructionHandler;
    use option::OptionInstructionHandler;
    use record::RecordInstructionHandler;
    use result::ResultInstructionHandler;
    use string::StringInstructionHandler;
    use variant::VariantInstructionHandler;

    let handlers: Vec<Box<dyn InstructionHandler>> = vec![
        Box::new(BasicInstructionHandler),
        Box::new(OptionInstructionHandler),
        Box::new(RecordInstructionHandler),
        Box::new(ResultInstructionHandler),
        Box::new(ListInstructionHandler),
        Box::new(VariantInstructionHandler),
        Box::new(StringInstructionHandler),
    ];

    for handler in handlers {
        if handler.can_handle(instruction) {
            return handler.handle(instruction, context, decoded);
        }
    }

    anyhow::bail!("Unhandled instruction: {:?}", instruction)
}
